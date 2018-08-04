#[macro_use]
extern crate failure;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate shrinkwraprs;
#[macro_use]
extern crate structopt;

extern crate ipc_channel;
extern crate libmount;
extern crate nix;
extern crate serde;
extern crate void;
extern crate yansi;

use failure::Error;
use std::process::exit;
use structopt::StructOpt;

mod colors;
mod container;
mod paths;
mod top_dirs;
mod utils;
mod zone;

use container::with_unshared_user_and_mount;
use paths::ZoneName;
use top_dirs::TopDirs;
use utils::execvp;
use zone::Zone;
use colors::color_err;

#[derive(StructOpt, Debug)]
#[structopt(name = "mzr", author = "Michael Sloan <mgsloan@gmail.com>")]
enum Cmd {
    #[structopt(name = "shell", about = "Enter a mizer shell")]
    Shell {},
}

fn main() {
    let cmd = Cmd::from_args();
    let result = match cmd {
        Cmd::Shell {} => shell(),
    };
    match result {
        Ok(()) => {}
        Err(err) => {
            println!("{} {:?}", color_err(&"Mizer error: "), err);
            exit(1);
        }
    }
}

fn shell() -> Result<(), Error> {
    let top_dirs = TopDirs::find_or_prompt_create("enter mizer shell")?;
    let zone_name = ZoneName::new("a-zone".to_string());
    let zone = Zone::load(&top_dirs, &zone_name)?;
    with_unshared_user_and_mount(|| {
        zone.mount()?;
        execvp("bash")?;
        Ok(())
    })?;
    Ok(())
}
