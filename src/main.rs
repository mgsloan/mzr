#![feature(try_blocks)]
#![feature(const_vec_new)]
#![feature(uniform_paths)]
#![warn(rust_2018_idioms)]

use structopt::StructOpt;
use mzr::*;
use std::process::exit;
use mzr::colors::color_err;

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
    #[structopt(
        name = "snap",
        about = "Create mzr snapshot of working directory"
    )]
    Snap {
        #[structopt(flatten)]
        opts: SnapOpts,
    },
    #[structopt(
        name = "go",
        about = "Switch working directory to a different zone"
    )]
    Go {
        #[structopt(flatten)]
        opts: GoOpts,
    },
}

pub fn main() {
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
            println!("Debug: {:?}", err);
            exit(1);
        }
    }
}
