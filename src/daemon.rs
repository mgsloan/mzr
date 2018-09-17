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

// TODO(correctness): Handshake should enforce version match
//
// TODO(correctness): Session types? :) Might be overkill

#[derive(Debug, Serialize, Deserialize)]
enum DaemonRequest {
    ZonePid(ZoneName),
}

#[derive(Debug, Serialize, Deserialize)]
enum DaemonResponse {
    ZonePid(ZonePid),
    Error(String),
}

fn handle_client(stream: UnixStream) -> Result<(), Error> {
    let mut data = Vec::new();
    let mut reader = BufReader::new(&stream);
    reader.read_until(b'\n', &mut data)?;
    let request: DaemonRequest = serde_json::from_slice(&data)?;
    println!("==> {:?}", request);
    let response = DaemonResponse::Error(String::from("works!"));
    serde_json::to_writer(&stream, &response)?;
    println!("<== {:?}", response);
    Ok(())
}

pub fn get_zone_process(mzr_dir: &MzrDir, zone_name: &ZoneName) -> Result<ZonePid, Error> {
    match send_to_daemon(mzr_dir, &DaemonRequest::ZonePid(zone_name.clone()))? {
        DaemonResponse::ZonePid(p) => Ok(p),
        x => bail!("Unexpected daemon response: {:?}", x),
    }
}

fn send_to_daemon(mzr_dir: &MzrDir, request: &DaemonRequest) -> Result<DaemonResponse, Error> {
    let daemon_dir = DaemonDir::new(&mzr_dir);
    let socket_path = DaemonSocketFile::new(&daemon_dir);
    if !socket_path.exists() {
        bail!(
            "Failed to connect to {}, because {} does not exist.",
            color_cmd(&String::from("mzr daemon")),
            socket_path
        );
    }
    let mut stream = UnixStream::connect(socket_path).context(format_err!(
        "Failed to connect to {}. Is it running?",
        color_cmd(&String::from("mzr daemon"))
    ))?;
    serde_json::to_writer(&stream, &request)?;
    stream.write_all(b"\n")?;
    Ok(serde_json::from_reader(&stream)?)
}
