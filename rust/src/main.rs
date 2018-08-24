#![feature(const_vec_new)]

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

extern crate ipc_channel;
extern crate libmount;
extern crate nix;
extern crate semver;
extern crate serde;
extern crate void;
extern crate yansi;

use failure::Error;
use std::process::exit;
use structopt::StructOpt;

mod colors;
mod container;
mod git;
mod paths;
mod snapshot;
mod top_dirs;
mod utils;
mod zone;

use colors::color_err;
use container::with_unshared_user_and_mount;
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
}

fn main() {
    let cmd = Cmd::from_args();
    let result = match cmd {
        Cmd::Shell { opts } => shell(opts),
        Cmd::Snap { opts } => snap(opts),
    };
    match result {
        Ok(()) => {}
        Err(err) => {
            println!("");
            println!("{} {}", color_err(&"mzr error:"), err);
            exit(1);
        }
    }
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

fn shell(opts: ShellOpts) -> Result<(), Error> {
    let top_dirs = TopDirs::find_or_prompt_create("enter mzr shell")?;
    let snap_name = default_git_snap_name(&top_dirs, opts.snap_name)?;
    let zone = Zone::load(&top_dirs, &opts.zone_name, &snap_name)?;
    with_unshared_user_and_mount(|| {
        zone.mount()?;
        execvp("bash")?;
        Ok(())
    })?;
    Ok(())
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

fn snap(opts: SnapOpts) -> Result<(), Error> {
    let top_dirs = TopDirs::find_or_prompt_create("take mzr snapshot")?;
    let snap_name = default_git_snap_name(&top_dirs, opts.snap_name)?;
    let _snap_dir = snapshot::of_workdir(&top_dirs, &snap_name)?;
    Ok(())
}

/*
 * Shared functions - things that are used by multiple commands, but seem to
 * belong in main.rs
 */

fn default_git_snap_name(
    top_dirs: &TopDirs,
    snap_name: Option<SnapName>,
) -> Result<SnapName, Error> {
    match snap_name {
        Some(name) => Ok(name),
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
