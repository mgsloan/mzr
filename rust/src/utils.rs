use failure::{Error, ResultExt};
use nix::unistd;
use std::ffi::CString;
use std::ffi::OsStr;
use std::io::{self, Write};
use std::path::PathBuf;
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
    match input.trim_right_matches("\n") {
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

// TODO: should handle args, will probably need that.
pub fn execvp(cmd: &str) -> Result<Void, Error> {
    let cmd_cstring = CString::new(cmd).context(format!(
        "Failed to convert command named {} to C string",
        cmd
    ))?;
    unistd::execvp(&cmd_cstring, &[]).context(format!(
        "Failed to execute bash. Is it in a directory listed in your PATH environment variable?"
    ))?;
    panic!("Impossible: execvp returned without an error code")
}
