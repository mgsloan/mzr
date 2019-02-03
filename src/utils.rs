use crate::colors::*;
use failure::{Error, Fail, ResultExt};
use nix::unistd;
use std::ffi::CString;
use std::ffi::OsStr;
use std::fmt::Display;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{exit, ExitStatus};
use std::process::{Command, Stdio};
use std::str::FromStr;
use void::Void;

/*
 * Console utilities
 */

pub enum Confirmed {
    Yes,
    No,
}

pub fn confirm(query: &str) -> Result<Confirmed, Error> {
    print!("{} [y/n]? ", query);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("Could not read stdin.")?;
    match input.trim_end_matches('\n') {
        "y" => Ok(Confirmed::Yes),
        "n" => Ok(Confirmed::No),
        other => Err(UnexpectedConfirmInput(other.to_string()).into()),
    }
}

#[derive(Fail, Debug)]
#[fail(display = "Expected 'y' or 'n' response.")]
struct UnexpectedConfirmInput(String);

/*
 * Path utilities
 */

pub fn add_suffix_to_path(path: &PathBuf, suffix: &str) -> PathBuf {
    match path.file_name().and_then(|x| x.to_str()) {
        Some(name) => {
            let mut result = path.clone();
            result.set_file_name(OsStr::new(&[name, suffix].concat()));
            result
        }
        None => panic!("Failed to add {} suffix to {}", suffix, path.display()),
    }
}

pub fn find_existent_parent_dir(path: &PathBuf) -> Option<PathBuf> {
    let mut dir = path.clone();
    while !dir.is_dir() {
        match dir.parent() {
            None => {
                return None;
            }
            Some(parent) => {
                dir = parent.to_path_buf();
            }
        }
    }
    Some(dir)
}

pub fn maybe_strip_prefix(prefix: &PathBuf, path: &PathBuf) -> PathBuf {
    path.strip_prefix(prefix).unwrap_or(path).to_path_buf()
}

/*
 * String utilities
 */

pub fn strip_prefix(prefix: &str, input: &str) -> Option<String> {
    if input.starts_with(prefix) {
        Some(input[prefix.len()..].to_string())
    } else {
        None
    }
}

/*
 * Process utilities
 */

/// Runs a process and yields an error if encountered.
pub fn run_process(cmd: &mut Command) -> Result<(), Error> {
    match cmd.status() {
        Err(e) => Err(e).context(format_err!(
            "Error encountered while running {:?}",
            color_cmd(cmd)
        ))?,
        Ok(status) => {
            if !status.success() {
                bail!(
                    "{:?} exited with failure status {}",
                    color_cmd(cmd),
                    color_err(&status)
                );
            }
        }
    }
    Ok(())
}

// TODO: should handle args, will probably need that.
pub fn execvp(cmd: &str) -> Result<Void, Error> {
    let cmd_cstring = CString::new(cmd).context(format!(
        "Failed to convert command named {} to C string",
        cmd
    ))?;
    unistd::execvp(&cmd_cstring, &[]).context(
        "Failed to execute bash. Is it in a directory listed in your PATH environment variable?",
    )?;
    panic!("Impossible: execvp returned without an error code")
}

/// Given an `ExitStatus`, probably yielded by an invoked process,
/// exits the current process with the same code.
pub fn exit_with_status(status: ExitStatus) -> Void {
    if status.success() {
        exit(0);
    } else {
        match status.code() {
            Some(code) => exit(code),
            None => match status.signal() {
                Some(signal) => exit(128 + signal),
                None => panic!("Failing exit status had neither code nor signal"),
            },
        }
    }
}

/*
 * File reading utilities
 */

pub fn parse_file<P: AsRef<Path> + Display, T: FromStr>(path: P) -> Result<T, Error>
where
    T::Err: Fail,
{
    let mut file = File::open(&path).context(format_err!("Failed to open {}", &path))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .context(format_err!("Failed to read {}", &path))?;
    Ok(contents.parse()?)
}

pub fn parse_pid_file<P: AsRef<Path> + Display>(path: P) -> Result<unistd::Pid, Error> {
    parse_file(path).map(unistd::Pid::from_raw)
}
