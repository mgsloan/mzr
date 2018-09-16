use crate::colors::*;
use crate::container;
use crate::paths::*;
use crate::zone::Zone;
use daemonize::Daemonize;
use failure::Error;
use std::fs::{create_dir_all, read_dir, File};
use std::path::PathBuf;
use std::thread;
use std::time;
use yansi::Paint;

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
