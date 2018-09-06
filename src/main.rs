#![feature(const_vec_new)]
// Allowing these makes prototyping convenient.
//
// TODO(cleanup): remove once that phase is done.
#![allow(dead_code)]
#![allow(unused_imports)]

#[macro_use]
extern crate failure;
#[macro_use]
extern crate failure_derive;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate shrinkwraprs;
#[macro_use]
extern crate structopt;

extern crate chrono;
extern crate daemonize;
extern crate ipc_channel;
extern crate libmount;
extern crate nix;
extern crate semver;
extern crate serde;
extern crate serde_json;
extern crate void;
extern crate yansi;

use failure::Error;
use std::env;
use std::process::exit;
use structopt::StructOpt;
use void::unreachable;

mod colors;
mod container;
mod daemon;
mod git;
mod json;
mod paths;
mod snapshot;
mod top_dirs;
mod utils;
mod zone;

use colors::color_err;
use paths::{SnapName, ZoneName};
use top_dirs::TopDirs;
use utils::execvp;
use zone::Zone;

/*
 * CLI options enum and main entrypoint
 */

#[derive(StructOpt, Debug)]
#[structopt(name = "mzr", author = "Michael Sloan <mgsloan@gmail.com>")]
enum Cmd {
    #[structopt(name = "daemon", about = "Run mzr daemon")]
    Daemon {},
    #[structopt(name = "shell", about = "Enter a mzr shell")]
    Shell {
        #[structopt(flatten)]
        opts: ShellOpts,
    },
    #[structopt(name = "snap", about = "Create mzr snapshot of working directory")]
    Snap {
        #[structopt(flatten)]
        opts: SnapOpts,
    },
    #[structopt(name = "go", about = "Switch working directory to a different zone")]
    Go {
        #[structopt(flatten)]
        opts: GoOpts,
    },
}

fn main() {
    let cmd = Cmd::from_args();
    let result = match cmd {
        Cmd::Daemon {} => daemon(),
        Cmd::Shell { opts } => shell(&opts),
        Cmd::Snap { opts } => snap(&opts),
        Cmd::Go { opts } => go(&opts),
    };
    match result {
        Ok(()) => {}
        Err(err) => {
            println!();
            println!("{} {}", color_err(&"mzr error:"), err);
            exit(1);
        }
    }
}

/*
 * "mzr daemon"
 */

fn daemon() -> Result<(), Error> {
    let top_dirs = TopDirs::find_or_prompt_create("start mzr daemon")?;
    daemon::run(&top_dirs.mzr_dir)
}

/*
 * "mzr shell"
 */

#[derive(StructOpt, Debug)]
struct ShellOpts {
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
    let zone = match Zone::load_if_exists(&top_dirs.mzr_dir, &opts.zone_name)? {
        Some(zone) => zone,
        None => {
            let snap_name = default_git_snap_name(&top_dirs, &opts.snap_name)?;
            /* TODO(friendliness): What should the snapshot creation logic be?
            println!("Taking a snapshot named {}", snap_name);
            snapshot::create(&top_dirs.user_work_dir, &top_dirs.mzr_dir, &snap_name)?;
            println!("Finished taking snapshot.");
            */
            Zone::create(&top_dirs.mzr_dir, &opts.zone_name, &snap_name)?
        }
    };
    container::enter_daemon_space(&top_dirs.mzr_dir)?;
    container::unshare_mount()?;
    zone.bind_to(&top_dirs.user_work_dir)?;
    env::set_current_dir(&top_dirs.user_work_dir)?;
    env::set_var("MZR_DIR", &top_dirs.mzr_dir);
    let void = execvp("bash")?;
    unreachable(void)
}

/*
 * "mzr snap"
 */

#[derive(StructOpt, Debug)]
struct SnapOpts {
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
    println!("Finished taking snapshot.");
    Ok(())
}

/*
 * "mzr go"
 */

#[derive(StructOpt, Debug)]
struct GoOpts {
    #[structopt(name = "ZONE_NAME", help = "Name of the zone to switch to.")]
    zone_name: ZoneName,
}

fn go(opts: &GoOpts) -> Result<(), Error> {
    let top_dirs = TopDirs::find("switch mzr zone")?;
    let zone = Zone::load(&top_dirs.mzr_dir, &opts.zone_name)?;
    // TODO: attempt to unmount old dir?  Would lead to a cleaner
    // mount list and notify when things are being used.
    //
    // TODO: ensure that we're in a mzr shell and that this zone is
    // mounted.
    zone.bind_to(&top_dirs.user_work_dir)
}

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
