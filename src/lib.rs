#![feature(try_blocks)]
#![feature(const_vec_new)]
#![feature(uniform_paths)]
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
mod namespaces;
mod paths;
mod snapshot;
mod top_dirs;
mod utils;
mod zone;

use crate::paths::{SnapName, ZoneName};
use crate::top_dirs::TopDirs;
use crate::utils::execvp;
use crate::zone::Zone;
use failure::Error;
use std::env;
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
        name = "snap",
        about = "Create mzr snapshot of working directory"
    )]
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
        Cmd::Snap { opts } => snap(&opts),
        // Cmd::Go { opts } => go(&opts),
    }
}

/*
 * "mzr daemon"
 */

fn daemon() -> Result<(), Error> {
    let top_dirs = TopDirs::find_or_prompt_create("start mzr daemon")?;
    daemon::run(&top_dirs)
}

/*
 * "mzr shell"
 */

#[derive(StructOpt, Debug)]
pub struct ShellOpts {
    #[structopt(
        name = "ZONE_NAME",
        help = "Name of the zone to load or create."
    )]
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
    let zone_pid = daemon::get_zone_process(&top_dirs.mzr_dir, &opts.zone_name)?;
    daemon::enter_zone_process_user_and_mount(&zone_pid)?;
    env::set_current_dir(&top_dirs.user_work_dir)?;
    env::set_var("MZR_DIR", &top_dirs.mzr_dir);
    let void = execvp("bash")?;
    unreachable(void)
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
