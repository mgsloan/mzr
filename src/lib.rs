#![feature(try_blocks)]
#![feature(const_vec_new)]
#![warn(rust_2018_idioms)]
// Allowing these makes prototyping convenient.
//
// TODO(cleanup): remove once that phase is done.
#![allow(dead_code)]
#![allow(unused_imports)]

// TODO(cleanup): figure out how to remove this
#[macro_use]
extern crate failure;

pub mod colors;
mod daemon;
mod git;
mod json;
mod merge;
mod namespaces;
mod paths;
mod snapshot;
mod top_dirs;
mod utils;
mod zone;

use crate::colors::color_dir;
use crate::merge::{interactive_merge, Mode};
use crate::paths::{SnapName, ZoneName};
use crate::top_dirs::TopDirs;
use crate::utils::{execvp, exit_with_status, find_existent_parent_dir, maybe_strip_prefix};
use crate::zone::Zone;
use failure::Error;
use nix::unistd::Pid;
use std::env;
use std::path::PathBuf;
use std::process::Command;
use structopt::StructOpt;
use void::unreachable;

/*
 * CLI options enum and runner
 */

#[derive(StructOpt, Debug)]
#[structopt(name = "mzr", author = "Michael Sloan <mgsloan@gmail.com>")]
pub enum Cmd {
    #[structopt(name = "daemon", about = "Run mzr daemon")]
    Daemon {},
    #[structopt(name = "shell", about = "Enter a mzr shell")]
    Shell {
        #[structopt(flatten)]
        opts: ShellOpts,
    },
    #[structopt(
        name = "run",
        about = "Run a command with a temporary snapshot and zone."
    )]
    Run {
        #[structopt(flatten)]
        opts: RunOpts,
    },
    #[structopt(name = "snap", about = "Create mzr snapshot of working directory")]
    Snap {
        #[structopt(flatten)]
        opts: SnapOpts,
    },
    /*
    #[structopt(
        name = "go",
        about = "Switch working directory to a different zone"
    )]
    Go {
        #[structopt(flatten)]
        opts: GoOpts,
    },
    */
}

pub fn run_cmd(cmd: &Cmd) -> Result<(), Error> {
    match cmd {
        Cmd::Daemon {} => daemon(),
        Cmd::Shell { opts } => shell(&opts),
        Cmd::Run { opts } => run(&opts),
        Cmd::Snap { opts } => snap(&opts),
        // Cmd::Go { opts } => go(&opts),
    }
}

/*
 * "mzr daemon"
 */

// TODO(friendliness): Perhaps other commands should automatically
// start daemon?  Ideally we wouldn't even need one, but it's not
// entirely clear to me how to do all the mount sharing without
// one. It may also be helpful in the future if a root daemon is
// supported (instead of using user namespaces).

fn daemon() -> Result<(), Error> {
    let top_dirs = TopDirs::find_or_prompt_create("start mzr daemon")?;
    daemon::run(&top_dirs)
}

/*
 * "mzr shell"
 */

#[derive(StructOpt, Debug)]
pub struct ShellOpts {
    #[structopt(name = "ZONE_NAME", help = "Name of the zone to load or create.")]
    zone_name: ZoneName,
    #[structopt(
        name = "SNAP_NAME",
        help = "Name of the snapshot to use. \
                If creating a new zone and this is unspecified, a new snapshot will be taken."
    )]
    snap_name: Option<SnapName>,
}

fn shell(opts: &ShellOpts) -> Result<(), Error> {
    let top_dirs = TopDirs::find_or_prompt_create("enter mzr shell")?;
    if !Zone::exists(&top_dirs.mzr_dir, &opts.zone_name) {
        let snap_name = default_git_snap_name(&top_dirs, &opts.snap_name)?;
        /* TODO(friendliness): What should the snapshot creation logic be?
        println!("Taking a snapshot named {}", snap_name);
        snapshot::create(&top_dirs.user_work_dir, &top_dirs.mzr_dir, &snap_name)?;
        println!("Finished taking snapshot.");
        */
        println!("Requested zone does not yet exist, so attempting to create it.");
        Zone::create(&top_dirs.mzr_dir, &opts.zone_name, &snap_name)?;
    };
    enter_zone(&top_dirs, &opts.zone_name)?;
    let void = execvp("/bin/bash")?;
    unreachable(void)
}

/*
 * "mzr run"
 */

#[derive(StructOpt, Debug)]
pub struct RunOpts {
    #[structopt(name = "CMD")]
    cmd: String,
    #[structopt(name = "ARGS")]
    args: Vec<String>,
}

fn run(opts: &RunOpts) -> Result<(), Error> {
    let top_dirs = TopDirs::find_or_prompt_create("run command in temp mzr zone")?;
    // TODO(friendliness) Things to consider basing tmp zone /
    // snapshot on:
    //
    // * Command run
    // * Current date / time
    // * Current PID
    //
    // For now just going with something based on PID..
    let tmp_name = format!("run-{}", Pid::this());
    let snap_name = SnapName::new(tmp_name.clone())?;
    let zone_name = ZoneName::new(tmp_name.clone())?;
    println!("Taking temporary snapshot named {}", snap_name);
    snapshot::of_workdir(&top_dirs, &snap_name)?;
    let zone = Zone::create(&top_dirs.mzr_dir, &zone_name, &snap_name)?;
    println!(
        "Running {} inside temporary zone named {}\n",
        opts.cmd, zone_name
    );
    // Run process within the temporary zone, inheriting stdio.
    enter_zone(&top_dirs, &zone_name)?;
    let mut child = Command::new(&opts.cmd).args(&opts.args).spawn()?;
    let status = child.wait()?;
    // TODO: I suppose the next steps here are:
    //
    // 1) Have this handled by the daemon, so that it has write access to the original working copy.
    //
    // 2) Know which zone 'run' is being invoked from, if any.
    //
    // 3) Summarize updates and display conflicts and skips. Ask about the conflicts and skips
    //
    // 4) Delete zone and snap if specified.
    //
    // 5) Should store in the zone and snap metadata that they are temporary.
    interactive_merge(
        &zone,
        top_dirs.user_work_dir.as_ref(),
        Mode::AutoApplyUpdates,
    )?;
    let _void = exit_with_status(status);
    unreachable(_void)
}

/*
 * "mzr snap"
 */

#[derive(StructOpt, Debug)]
pub struct SnapOpts {
    #[structopt(
        name = "SNAP_NAME",
        help = "Name of the snapshot to create. \
                If unspecified, a name will be generated based on the current git branch name."
    )]
    snap_name: Option<SnapName>,
}

fn snap(opts: &SnapOpts) -> Result<(), Error> {
    let top_dirs = TopDirs::find_or_prompt_create("take mzr snapshot")?;
    let snap_name = default_git_snap_name(&top_dirs, &opts.snap_name)?;
    println!("Taking a snapshot named {}", snap_name);
    let _snap_dir = snapshot::of_workdir(&top_dirs, &snap_name)?;
    println!(
        "{} snapshot named {} taken.",
        colors::color_success(&"Success:"),
        snap_name
    );
    Ok(())
}

/*
 * "mzr go"
 */

// TODO(feature): Should bring back "mzr go", this code worked back
// when the user in the shell was already root.

/*
#[derive(StructOpt, Debug)]
pub struct GoOpts {
    #[structopt(name = "ZONE_NAME", help = "Name of the zone to switch to.")]
    zone_name: ZoneName,
}

fn go(opts: &GoOpts) -> Result<(), Error> {
    let top_dirs = TopDirs::find("switch mzr zone")?;
    let zone = Zone::load(&top_dirs.mzr_dir, &opts.zone_name)?;
    // Ask daemon to start zone process, to ensure that the overlay
    // gets mounted.
    daemon::get_zone_process(&top_dirs.mzr_dir, &opts.zone_name)?;
    // TODO: attempt to unmount old dir?  Would lead to a cleaner
    // mount list and notify when things are being used.
    //
    // TODO: ensure that we're in a mzr shell and that this zone is
    // mounted.
    zone.bind_to(&top_dirs.user_work_dir)
}
*/

/*
 * Shared functions - things that are used by multiple commands, but seem to
 * belong in main.rs
 */

fn default_git_snap_name(
    top_dirs: &TopDirs,
    snap_name: &Option<SnapName>,
) -> Result<SnapName, Error> {
    match snap_name {
        Some(name) => Ok(name.clone()),
        None => {
            git::warn_env();
            // TODO: Consider adding "_vN" suffixes to these, to disambiguate
            // with existing snapshots.
            let name = git::default_snap_name(&top_dirs.user_work_dir)?;
            println!(
                "Since no snapshot was specified, using the current git ref or sha: {}",
                name
            );
            Ok(name)
        }
    }
}

fn enter_zone(top_dirs: &TopDirs, zone_name: &ZoneName) -> Result<(), Error> {
    let current_directory = env::current_dir()?;
    let zone_pid = daemon::get_zone_process(&top_dirs.mzr_dir, &zone_name)?;
    daemon::enter_zone_process_user_and_mount(&zone_pid)?;
    change_dir_fallback_parent(&top_dirs.user_work_dir, &current_directory)?;
    env::set_var("MZR_DIR", &top_dirs.mzr_dir);
    Ok(())
}

fn change_dir_fallback_parent(
    work_dir: &paths::UserWorkDir,
    start_dir: &PathBuf,
) -> Result<(), Error> {
    match find_existent_parent_dir(start_dir) {
        Some(existent_dir) => {
            if &existent_dir != start_dir {
                println!(
                    "Couldn't find {:?} in zone, so instead setting current directory to {:?}",
                    maybe_strip_prefix(&work_dir, &existent_dir),
                    existent_dir
                );
            }
            env::set_current_dir(existent_dir)?;
            Ok(())
        }
        None => {
            bail!("Couldn't find existent parent of old CWD {:?}", start_dir);
        }
    }
}
