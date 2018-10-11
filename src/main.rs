#![feature(try_blocks)]
#![feature(const_vec_new)]
#![feature(uniform_paths)]
#![warn(rust_2018_idioms)]

use mzr::colors::color_err;
use mzr::*;
use std::process::exit;
use structopt::StructOpt;

pub fn main() {
    match run_cmd(&Cmd::from_args()) {
        Ok(()) => {}
        Err(err) => {
            println!();
            println!("{} {}", color_err(&"mzr error:"), err);
            println!("Debug: {:?}", err);
            exit(1);
        }
    }
}
