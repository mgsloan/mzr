use crate::colors::*;
use crate::namespaces;
use crate::paths::*;
use crate::top_dirs::TopDirs;
use crate::zone::Zone;
use daemonize::Daemonize;
use failure::{Error, ResultExt};
use libc::pid_t;
use nix::unistd::Pid;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{create_dir_all, read_dir, remove_file, File};
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
}

type ProcessMap = HashMap<ZoneName, ZonePid>;

pub fn run(top_dirs: &TopDirs) -> Result<(), Error> {
    let _pid = namespaces::with_unshared_user_and_mount(|| {
        let daemon_dir = DaemonDir::new(&top_dirs.mzr_dir);
        create_dir_all(&daemon_dir)?;
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
            match handle_client(&top_dirs, stream, &mut processes) {
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
    })?;
    // TODO(friendliness): Include this output, but only do it when
    // the daemon has actually started. Currently if you start the
    // daemon while another is running, and this line is uncommented,
    // it outputs.
    //
    // println!("Started {} with PID {}", color_cmd(&String::from("mzr daemon")), color_cmd(&pid));
    Ok(())
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
    stream: UnixStream,
    processes: &mut ProcessMap,
) -> Result<(), Error> {
    let result: Result<Response, Error> = try {
        match recv_request(&stream)? {
            Request::ZoneProcess(zone_name) => match processes.get(&zone_name) {
                None => match Zone::load_if_exists(&top_dirs.mzr_dir, &zone_name)? {
                    None => Response::Error(String::from("Zone does not exist")),
                    Some(zone) => {
                        // Mount the zone's overlayfs in the daemon's namespace.
                        //
                        // TODO: Looks like this does not yet
                        // propagate to the mount namespaces of the
                        // existing zone processes, but it needs to.
                        zone.mount()?;
                        // Fork a zone process which bind-mounts the
                        // zone to the user's working directory.
                        let pid = fork_zone_process(&top_dirs.user_work_dir, &zone)?;
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

fn fork_zone_process(work_dir: &UserWorkDir, zone: &Zone) -> Result<ZonePid, Error> {
    // TODO(cleanup): mzr now has a few different takes on IPC, should
    // use a consistent style.
    let (server_stream, mut client_stream) = UnixStream::pair()?;
    let pid = namespaces::with_unshared_mount(|| {
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
    })?;
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
        Ok(ZonePid(pid_t::from(pid)))
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

fn send_to_daemon(mzr_dir: &MzrDir, request: &Request) -> Result<Response, Error> {
    let daemon_dir = DaemonDir::new(&mzr_dir);
    let socket_path = DaemonSocketFile::new(&daemon_dir);
    if !socket_path.exists() {
        bail!(
            "Failed to connect to {}, because {} does not exist.",
            color_cmd(&String::from("mzr daemon")),
            socket_path
        );
    }
    let stream = UnixStream::connect(socket_path).context(format_err!(
        "Failed to connect to {}. Is it running?",
        color_cmd(&String::from("mzr daemon"))
    ))?;
    send_request(&stream, request)?;
    recv_response(&stream)
}

pub fn get_zone_process(mzr_dir: &MzrDir, zone_name: &ZoneName) -> Result<ZonePid, Error> {
    match send_to_daemon(mzr_dir, &Request::ZoneProcess(zone_name.clone()))? {
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
