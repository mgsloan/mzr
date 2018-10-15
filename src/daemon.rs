use crate::colors::*;
use crate::git::{get_git_dir, symlink_git_repo};
use crate::namespaces;
use crate::paths::*;
use crate::top_dirs::TopDirs;
use crate::zone::Zone;
use daemonize::Daemonize;
use failure::{Error, ResultExt};
use libc::pid_t;
use libmount::BindMount;
use nix::unistd::{Gid, Pid, Uid};
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::fs::{create_dir, create_dir_all, read_dir, remove_file, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::thread;
use std::time;
use yansi::Paint;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DaemonPid(pid_t);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ZonePid(pid_t);

impl ZonePid {
    pub fn to_pid(&self) -> Pid {
        Pid::from_raw(self.0)
    }

    pub fn from_pid(p: Pid) -> Self {
        ZonePid(pid_t::from(p))
    }
}

impl Display for ZonePid {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        color_zone_pid(&self.0).fmt(f)
    }
}

type ProcessMap = HashMap<ZoneName, ZonePid>;

pub fn run(top_dirs: &TopDirs) -> Result<(), Error> {
    let user = Uid::current();
    let group = Gid::current();
    let _pid = namespaces::with_unshared_user_and_mount(
        |child_process| namespaces::map_user_to_root(child_process, user, group),
        || {
            let daemon_dir = DaemonDir::new(&top_dirs.mzr_dir);
            create_dir_all(&daemon_dir)?;
            let git_info = bind_git_repo(top_dirs)?;
            // TODO(cleanup): Don't truncate old daemon logs?
            let log_file = File::create(DaemonLogFile::new(&daemon_dir))?;
            Daemonize::new()
                .pid_file(DaemonPidFile::new(&daemon_dir))
                .stdout(log_file)
                .start()?;
            // Disable ANSI codes in output, since it's sent to a log
            // rather than terminal.
            Paint::disable();
            // Listen for client connections.
            let socket_path = DaemonSocketFile::new(&daemon_dir);
            if socket_path.exists() {
                remove_file(&socket_path).context(format_err!(
                    "Failed to remove daemon socket file {}",
                    socket_path
                ))?;
            }
            // Mutable hashmap to track which child processes have been
            // created.
            let mut processes = HashMap::new();
            // Listen for client connections. In the future, perhaps tokio
            // or mio will be used, but for now using the lower level APIs
            // because they are simpler and have better documentation.
            let listener = UnixListener::bind(socket_path)?;
            for stream_or_err in listener.incoming() {
                let stream = stream_or_err?;
                match handle_client(&top_dirs, &git_info, user, group, stream, &mut processes) {
                    Ok(()) => (),
                    Err(err) => {
                        println!("");
                        println!("Error while handling client.");
                        println!("Debug info for exception: {:?}", err);
                        println!("Display info for exception: {}", err);
                        println!("Ignoring this and continuing daemon execution...");
                        println!("");
                    }
                }
            }
            Ok(())
        },
    )?;
    // TODO(friendliness): Include this output, but only do it when
    // the daemon has actually started. Currently if you start the
    // daemon while another is running, and this line is uncommented,
    // it outputs.
    //
    // println!("Started {} with PID {}", color_cmd(&String::from("mzr daemon")), color_cmd(&pid));
    Ok(())
}

// If there is a top level git repository, bind mount it, so that the
// repo can be shared by the zones.
//
// TODO(correctness): This is gnarly. Instead, git repos should be
// supported after the daemon has already started. Should also support
// multiple git repos.
fn bind_git_repo(
    top_dirs: &TopDirs,
) -> Result<Option<(BoundGitRepoDir, RelativeGitRepoDir)>, Error> {
    Ok(match get_git_dir(&top_dirs.user_work_dir) {
        Err(_) => None,
        Ok(rel_git_dir) => {
            let src_git_dir = top_dirs.user_work_dir.join(&rel_git_dir);
            if src_git_dir.is_dir() {
                let bound_git_repo_dir = BoundGitRepoDir::new(&top_dirs.mzr_dir);
                create_dir_all(&bound_git_repo_dir)?;
                BindMount::new(&src_git_dir, &bound_git_repo_dir)
                    .mount()
                    .map_err(|e| format_err!("{}", e))?;
                Some((bound_git_repo_dir, rel_git_dir))
            } else {
                None
            }
        }
    })
}

/*
 * Types for daemon <==> client communication
 */

// TODO(correctness): Handshake should enforce version match.

#[derive(Debug, Serialize, Deserialize)]
enum Request {
    ZoneProcess(ZoneName),
}

#[derive(Debug, Serialize, Deserialize)]
enum Response {
    ZoneProcess(ZonePid),
    Error(String),
}

/*
 * Handler for a client connection
 */

fn handle_client(
    top_dirs: &TopDirs,
    git_info: &Option<(BoundGitRepoDir, RelativeGitRepoDir)>,
    user: Uid,
    group: Gid,
    stream: UnixStream,
    processes: &mut ProcessMap,
) -> Result<(), Error> {
    let result: Result<Response, Error> = try {
        match recv_request(&stream)? {
            Request::ZoneProcess(zone_name) => match processes.get(&zone_name) {
                None => match Zone::load_if_exists(&top_dirs.mzr_dir, &zone_name)? {
                    None => Response::Error(String::from("Zone does not exist")),
                    Some(zone) => {
                        match git_info {
                            None => {}
                            Some((source_git_dir, rel_git_dir)) => {
                                let target_git_dir = zone.ovfs_changes_dir.join(rel_git_dir);
                                symlink_git_repo(&source_git_dir, &target_git_dir)?;
                            }
                        }
                        // Mount the zone's overlayfs in the daemon's namespace.
                        //
                        // TODO: Looks like this does not yet
                        // propagate to the mount namespaces of the
                        // existing zone processes, but it needs to.
                        zone.mount()?;
                        // Fork a zone process which bind-mounts the
                        // zone to the user's working directory.
                        let pid = fork_zone_process(&top_dirs.user_work_dir, user, group, &zone)?;
                        processes.insert(zone_name, pid.clone());
                        Response::ZoneProcess(pid)
                    }
                },
                Some(pid) => Response::ZoneProcess(pid.clone()),
            },
        }
    };
    send_response(
        &stream,
        &match result {
            Ok(x) => x,
            Err(e) => Response::Error(format!("Unexpected error: {}", e)),
        },
    )
}

const READY_MSG: &[u8; 6] = b"ready\n";

fn fork_zone_process(
    work_dir: &UserWorkDir,
    user: Uid,
    group: Gid,
    zone: &Zone,
) -> Result<ZonePid, Error> {
    // TODO(cleanup): mzr now has a few different takes on IPC, should
    // use a consistent style.
    let (server_stream, mut client_stream) = UnixStream::pair()?;
    let pid = namespaces::with_unshared_user_and_mount(
        |child_process| namespaces::map_root_to_user(child_process, user, group),
        || {
            // TODO(cleanup): When the parent process exits, it should
            // close the pipe, which should cause the read to
            // exit. However, for some reason that didn't work. Setting
            // PDEATHSIG seems to work, though. It would be nicer to avoid
            // this, though.
            unsafe {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0 {
                    bail!("Failed to set PDEATHSIG");
                }
            }
            // Bind mount zone over the user's work-dir.
            zone.bind_to(work_dir)?;
            // Indicate to parent process that the zone is ready.
            client_stream.write_all(READY_MSG)?;
            let mut data = Vec::new();
            // This should just block forever, since server_stream never
            // gets written to.
            let result = client_stream.read_to_end(&mut data);
            println!(
                "mzr zone process unexpectedly done blocking, result was {:?}",
                result
            );
            Ok(())
        },
    )?;
    let mut data = Vec::new();
    let mut reader = BufReader::new(server_stream);
    reader.read_until(b'\n', &mut data)?;
    if data != READY_MSG {
        Err(format_err!(
            "Didn't receive expected message from child process. Instead got {:?}",
            data
        ))
    } else {
        println!("Zone process forked for zone named \"{}\"", zone.name);
        Ok(ZonePid::from_pid(pid))
    }
}

/*
 * Functions for daemon receiving requests and sending responses.
 */

fn recv_request(stream: &UnixStream) -> Result<Request, Error> {
    let mut data = Vec::new();
    let mut reader = BufReader::new(stream);
    reader.read_until(b'\n', &mut data)?;
    let request: Request = serde_json::from_slice(&data)?;
    println!("==> {:?}", request);
    Ok(request)
}

fn send_response(stream: &UnixStream, response: &Response) -> Result<(), Error> {
    serde_json::to_writer(stream, &response)?;
    println!("<== {:?}", response);
    Ok(())
}

/*
 * Functions for client sending requests and receiving responses.
 */

fn send_request(mut stream: &UnixStream, request: &Request) -> Result<(), Error> {
    serde_json::to_writer(stream, request)?;
    stream.write_all(b"\n")?;
    Ok(())
}

fn recv_response(stream: &UnixStream) -> Result<Response, Error> {
    Ok(serde_json::from_reader(stream)?)
}

fn connect_to_daemon(mzr_dir: &MzrDir) -> Result<UnixStream, Error> {
    let daemon_dir = DaemonDir::new(&mzr_dir);
    let socket_path = DaemonSocketFile::new(&daemon_dir);
    if !socket_path.exists() {
        bail!(
            "Failed to connect to {}, because {} does not exist.",
            color_cmd(&String::from("mzr daemon")),
            socket_path
        );
    }
    Ok(UnixStream::connect(socket_path).context(format_err!(
        "Failed to connect to {}. Is it running?",
        color_cmd(&String::from("mzr daemon"))
    ))?)
}

fn run_daemon_command(mzr_dir: &MzrDir, request: &Request) -> Result<Response, Error> {
    let stream = connect_to_daemon(mzr_dir)?;
    send_request(&stream, request)?;
    recv_response(&stream)
}

pub fn get_zone_process(mzr_dir: &MzrDir, zone_name: &ZoneName) -> Result<ZonePid, Error> {
    let request = Request::ZoneProcess(zone_name.clone());
    // TODO(hack): Sending the request twice is an ugly hack. For some
    // reason, on initial forking of the daemon's zone process, the
    // response never makes it back to the client. I suspect this is
    // related to the client process getting control of the the
    // stream, but it seems like FD_CLOEXEC is being set in the
    // code.
    //
    // The workaround here is to ask twice, and use the response from
    // the 2nd request, since that will just be a lookup in the
    // daemon's cache.
    let stream = connect_to_daemon(mzr_dir)?;
    send_request(&stream, &request)?;
    // Make the request again to actually get the process.
    match run_daemon_command(mzr_dir, &request)? {
        Response::ZoneProcess(p) => Ok(p),
        Response::Error(e) => bail!("Response from daemon was {:?}", e),
    }
}

/*
 * Functions for entering zone process namespaces.
 */

pub fn enter_zone_process_mount(zone_pid: &ZonePid) -> Result<(), Error> {
    namespaces::enter_mount(zone_pid.to_pid())
}

pub fn enter_zone_process_user_and_mount(zone_pid: &ZonePid) -> Result<(), Error> {
    namespaces::enter_user_and_mount(zone_pid.to_pid())
}
