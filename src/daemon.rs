use crate::colors::*;
use crate::container;
use crate::paths::*;
use crate::zone::Zone;
use daemonize::Daemonize;
use failure::{Error, ResultExt};
use libc::pid_t;
use nix::unistd::Pid;
use serde_derive::{Deserialize, Serialize};
use std::fs::{create_dir_all, read_dir, remove_file, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::thread;
use std::time;
use yansi::Paint;

#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonPid(pid_t);

#[derive(Debug, Serialize, Deserialize)]
pub struct ZonePid(pid_t);

pub fn run(mzr_dir: &MzrDir) -> Result<(), Error> {
    let _pid = container::with_unshared_user_and_mount(|| {
        let daemon_dir = DaemonDir::new(&mzr_dir);
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
        // Listen for client connections. In the future, perhaps tokio
        // or mio will be used, but for now using the lower level APIs
        // because they are simpler and have better documentation.
        let listener = UnixListener::bind(socket_path)?;
        for stream_or_err in listener.incoming() {
            let stream = stream_or_err?;
            thread::spawn(|| match handle_client(stream) {
                Ok(()) => (),
                Err(err) => {
                    println!("");
                    println!("Error while handling client.");
                    println!("Debug info for exception: {:?}", err);
                    println!("Display info for exception: {}", err);
                    println!("Ignoring this and continuing daemon execution...");
                    println!("");
                }
            });
        }
        /*
        // Mount all zones
        let mzr_dir_buf: &PathBuf = mzr_dir.as_ref();
        let mut zone_parent_dir = mzr_dir_buf.clone();
        zone_parent_dir.push("zone");
        for entry in read_dir(zone_parent_dir)? {
            let zone_name = ZoneName::new(entry?.file_name().into_string().unwrap())?;
            let zone = Zone::load(&mzr_dir, &zone_name)?;
            println!("Mounting overlay for zone named {}", zone_name);
            zone.mount()?;
        }
        // FIXME: Obviously this will need to change
        println!("Sleeping for an hour...");
        thread::sleep(time::Duration::from_secs(60 * 60));
        */
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

// TODO(correctness): Handshake should enforce version match

#[derive(Debug, Serialize, Deserialize)]
enum Request {
    ZonePid(ZoneName),
}

#[derive(Debug, Serialize, Deserialize)]
enum Response {
    ZonePid(ZonePid),
    Error(String),
}

/*
 * Handler for a client connection
 */

fn handle_client(stream: UnixStream) -> Result<(), Error> {
    let result: Result<Response, Error> = try {
        match recv_request(&stream)? {
            Request::ZonePid(name) => {
                Response::Error(String::from("fixme"))
            }
        }
    };
    send_response(&stream, &match result {
        Ok(x) => x,
        Err(e) => Response::Error(format!("Unexpected error: {}", e)),
    })
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
    match send_to_daemon(mzr_dir, &Request::ZonePid(zone_name.clone()))? {
        Response::ZonePid(p) => Ok(p),
        Response::Error(e) => bail!("Response from daemon was {:?}", e),
    }
}
