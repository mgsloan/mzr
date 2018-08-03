#[macro_use]
extern crate structopt;
extern crate nix;
extern crate yansi;

use std::{thread, time};
use nix::sched::{unshare, CloneFlags};
use nix::unistd;
use std::env::{current_dir, current_exe};
use std::ffi::{CString, OsStr};
use std::fs::{canonicalize, create_dir, OpenOptions, File};
use std::io::{self, Read, Write};
use std::os::unix::io::{RawFd, FromRawFd};
use std::path::PathBuf;
use std::process;
use structopt::StructOpt;
use yansi::Paint;

const MIZER_SUFFIX: &str = ".mizer";

// FIXME: remove all usage of unwrap / panic.

#[derive(StructOpt, Debug)]
#[structopt(name = "mzr", author = "Michael Sloan <mgsloan@gmail.com>")]
enum Cmd {
    #[structopt(name = "shell", about = "Enter a mizer shell")]
    Shell {},
    // TODO: Hide from user
    #[structopt(name = "child")]
    Child { read_fd: RawFd },
    /*
    #[structopt(name = "init", about = "Initialize a mizer directory")]
    Init {
        #[structopt(parse(from_os_str))]
        explicit_target: Option<PathBuf>,
    },
    #[structopt(name = "snap", about = "Use rsync to create a snapshot")]
    Snap {},
    #[structopt(name = "go", about = "Switch mizer context")]
    Go {},
    #[structopt(name = "ls", about = "List mizer contexts and snapshots")]
    List {},
    #[structopt(name = "rm", about = "Remove mizer context")]
    Remove {},
*/
}

fn main() {
    let cmd = Cmd::from_args();
    println!("{:?}", cmd);
    match cmd {
        Cmd::Shell {} => {
            shell(find_or_prompt_mizer_dir("enter mizer shell"));
        }
        Cmd::Child { read_fd } => {
            println!("Made it to child, read_fd is {}", read_fd);
            unsafe {
                let mut read_file = File::from_raw_fd(read_fd);
                println!("before read");
                let mut contents = String::new();
                read_file.read_to_string(&mut contents);
                println!("file contents is {}", contents);
            }
        } /*
        Cmd::Init { explicit_target } => {
            let target = match explicit_target {
                None => current_dir().unwrap(),
                Some(x) => {
                    if !x.is_dir() {
                        panic!("Argh");
                    }
                    x
                }
            };
            init(canonicalize(target).unwrap()).unwrap();
        }
        Cmd::Snap {} => {
            snap();
        }
        Cmd::Go {} => {
            go();
        }
        Cmd::List {} => {}
        Cmd::Remove {} => {}
*/
    }
}

fn shell(dirs: MizerDirs) {
    println!("Entering shell with {:?}", dirs);

    /* We use a pipe to synchronize the parent and child, in order to
    ensure that the parent sets the UID and GID maps before the child
    calls execve(). This ensures that the child maintains its
    capabilities during the execve() in the common case where we
    want to map the child's effective user ID to 0 in the new user
    namespace. Without this synchronization, the child would lose
    its capabilities if it performed an execve() with nonzero
    user IDs (see the capabilities(7) man page for details of the
    transformation of a process's capabilities during execve()). */
    let (read_fd, write_fd) = unistd::pipe().unwrap();

    let child = process::Command::new(current_exe().unwrap())
        .arg("child")
        .arg(read_fd.to_string())
        .spawn()
        .unwrap();

    println!("write_fd is {}", write_fd);

    unsafe {
        File::from_raw_fd(read_fd);
    }

    thread::sleep(time::Duration::from_millis(1000));

    unsafe {
        let mut write_file = File::from_raw_fd(write_fd);
        write_file.write_all(b"ready").unwrap();
    }
}

/*
fn shell(dirs: MizerDirs) {
    println!("Entering shell with {:?}", dirs);
    // Unshare the mount and user namespaces.
    unshare(CloneFlags::CLONE_NEWNS | CloneFlags::CLONE_NEWUSER);
    // Map current user to root within our new user namespace.
    let pid = process::id();
    let uid_map_path = format!("/proc/{}/uid_map", pid);
    let mut uid_map_file = OpenOptions::new().write(true).open(uid_map_path).unwrap();
    uid_map_file.write_all(format!("0 {} 1\n", unistd::Uid::current()).as_bytes());
    uid_map_file.sync_all();
    execvp(&CString::new("bash").unwrap(), &[]);
}
*/

/*
fn init(target: PathBuf) -> std::io::Result<()> {
    let mizer_dir = mizer_dir_from_work_dir(&target);
    if mizer_dir.is_dir() {
        // TODO: more checking
        println!(
            "mizer directory {:?} already exists, so doing nothing.",
            Paint::blue(mizer_dir)
        );
        return Ok(());
    } else if mizer_dir.is_file() {
        panic!(
            "{} location for mizer directory {:?}",
            Paint::red("Error: "),
            Paint::blue(mizer_dir)
        );
    }
    create_dir(&mizer_dir)?;
    println!(
        "Initialized empty mizer directory in {:?}.",
        Paint::blue(mizer_dir)
    );
    Ok(())
}

fn snap() {
    let mizer_dir = find_mizer_dir();
    println!("mizer dir = {:?}", mizer_dir);
    // create_dir()
}

fn go() {
}
*/

// Utilities

#[derive(Debug)]
struct MizerDirs {
    mizer_dir: PathBuf,
    work_dir: PathBuf,
}

fn pretty_mizer_dir(dirs: &MizerDirs) -> yansi::Paint<&PathBuf> {
    return Paint::blue(&dirs.mizer_dir);
}

fn find_mizer_dirs() -> Option<MizerDirs> {
    let mut dir = current_dir().unwrap();
    loop {
        let candidate_dirs = mizer_dirs_from_work_dir(&dir);
        if candidate_dirs.mizer_dir.is_dir() {
            return Some(candidate_dirs);
        }
        dir.pop();
        if dir.file_name().is_none() {
            return None;
        }
    }
}

fn mizer_dirs_from_work_dir(work_dir: &PathBuf) -> MizerDirs {
    let mut mizer_dir = work_dir.clone();
    let base_name = work_dir.file_name().unwrap().to_str().unwrap();
    mizer_dir.set_file_name(OsStr::new(&[base_name, MIZER_SUFFIX].concat()));
    MizerDirs {
        mizer_dir,
        work_dir: work_dir.to_path_buf(),
    }
}

fn find_or_prompt_mizer_dir(action: &str) -> MizerDirs {
    match find_mizer_dirs() {
        None => {
            let dirs = mizer_dirs_from_work_dir(&current_dir().unwrap());
            println!("Couldn't find a mizer dir sibling to any parent directories.");
            match confirm(format!(
                "Initialize a new mizer directory at {:?}",
                pretty_mizer_dir(&dirs)
            )) {
                None => panic!("{} Unexpected input, exiting.", Paint::red("Error: ")),
                Some(false) => panic!("Can't {} without a mizer directory", action),
                Some(true) => {
                    // TODO: Why is this clone needed?
                    create_dir(dirs.mizer_dir.clone()).unwrap();
                    println!("Mizer directory initialized.");
                    dirs
                }
            }
        }
        Some(x) => x,
    }
}

fn confirm(query: String) -> Option<bool> {
    print!("{} [y/n]? ", query);
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    match input.trim_right_matches("\n") {
        "y" => Some(true),
        "n" => Some(false),
        _ => None,
    }
}

/* Consider doing this instead of exec-ing again. Eh, probably not a big deal.

fn process_clone(cb: () -> ()) {
    let stack_size = 1024 * 1024;
    let ptr = heap::allocate(1024 * 1024, 8);
    libc::clone(cb, )
}
*/
