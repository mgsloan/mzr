use crate::colors::*;
use crate::paths::{BoundGitRepoDir, RelativeGitRepoDir, SnapName, UserWorkDir};
use crate::utils::strip_prefix;
use failure::{Error, ResultExt};
use semver::Version;
use std::env;
use std::fmt;
use std::fs::{create_dir_all, read_link};
use std::io::ErrorKind;
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};

// This implements something very similar to git's old "workdir"
// approach for having multiple working directories associated with
// one repository.
//
// Unlike the script there, this is idempotent, but only if the
// symlinks are correct.
pub fn symlink_git_repo(source_git_dir: &PathBuf, target_git_dir: &PathBuf) -> Result<(), Error> {
    // Based on list / code at
    // https://github.com/git/git/blob/e32afab7b0376a7b07601a87cd5c6841ff2a811a/contrib/workdir/git-new-workdir#L82
    for shared_path in [
        "config",
        "refs",
        "logs/refs",
        "objects",
        "info",
        "hooks",
        "packed-refs",
        "remotes",
        "rr-cache",
        "svn",
    ]
    .iter()
    {
        let source_path = source_git_dir.join(shared_path);
        let target_path = target_git_dir.join(shared_path);
        let possibly_existing_link = read_link(&target_path).context(format_err!(
            "Expected {:?} to be a symbolic link.",
            &target_path
        ));
        match possibly_existing_link {
            Err(_) => {
                create_dir_all(target_path.parent().unwrap())?;
                // Note that the source path does not need to exist.  For
                // example the 'svn' dir probably usually doesn't exist.
                symlink(&source_path, &target_path).context(format_err!(
                    "Failed to create git repo symlink at {:?}, pointing to {:?}",
                    target_path,
                    source_path
                ))?;
            }
            Ok(existing_link) => {
                if existing_link != source_path {
                    bail!(
                        "Expected {:?} to be a symbolic link to {:?}, but instead it points at {:?}",
                        &target_path,
                        &source_path,
                        &existing_link
                    );
                }
            }
        }
    }
    Ok(())
}

pub fn default_snap_name(work_dir: &UserWorkDir) -> Result<SnapName, Error> {
    match current_ref_or_short_sha(&work_dir) {
        Err(e) => Err(format_err!(
            "Since no snapshot was specified, attempted to query git for \
             current ref or sha info. Encountered an error:\n{}",
            e
        )),
        Ok(raw_name) => match SnapName::new(raw_name.clone()) {
            Err(e) => Err(format_err!(
                "Since no snapshot was specified, queried git for \
                 current ref or sha info.  There was an error parsing \
                 the resulting git ref \"{}\" as a snapshot name:\n{}",
                raw_name,
                e
            )),
            Ok(name) => Ok(name),
        },
    }
}

fn current_ref_or_short_sha(work_dir: &UserWorkDir) -> Result<String, GitError> {
    match symbolic_ref_short(work_dir) {
        Ok(result) => Ok(result),
        Err(e) => match e {
            GitError::ExitStatus(cmd, output, status) => {
                // NOTE: would rather use the status code, but oddly enough
                // 32768 is reported instead of what I get in bash, 128. So
                // going to just match on message instead.
                if output.ends_with("is not a symbolic ref\n") {
                    let sha = head_sha(work_dir)?;
                    Ok(sha[..6].to_string())
                } else {
                    Err(GitError::ExitStatus(cmd, output, status))
                }
            }
            // Other errors are unexpected, and so should be yielded for debugging
            // purposes. Better than unexpectedly falling back on SHA, I think.
            _ => Err(e),
        },
    }
}

fn symbolic_ref_short(work_dir: &UserWorkDir) -> Result<String, GitError> {
    collect_output(
        Command::new("git")
            .stdin(Stdio::null())
            .current_dir(work_dir)
            .arg("symbolic-ref")
            .arg("--short")
            .arg("HEAD"),
    )
    .map(|x| x.trim().to_string())
}

fn head_sha(work_dir: &UserWorkDir) -> Result<String, GitError> {
    collect_output(
        Command::new("git")
            .stdin(Stdio::null())
            .current_dir(work_dir)
            .arg("rev-parse")
            .arg("HEAD"),
    )
    .map(|x| x.trim().to_string())
}

pub fn get_git_dir(work_dir: &UserWorkDir) -> Result<RelativeGitRepoDir, GitError> {
    collect_output(
        Command::new("git")
            .stdin(Stdio::null())
            .current_dir(work_dir)
            .arg("rev-parse")
            .arg("--git-dir"),
    )
    .map(|x| RelativeGitRepoDir::new(x.trim()))
}

fn collect_output(cmd: &mut Command) -> Result<String, GitError> {
    match collect_output_base(cmd) {
        Err(err) => Err(err),
        Ok((status, stdout, stderr)) => {
            if status.success() {
                Ok(stdout)
            } else {
                match check_version() {
                    Err(e @ GitError::TooOld(_)) => Err(e),
                    _ => Err(GitError::ExitStatus(format!("{:?}", cmd), stderr, status)),
                }
            }
        }
    }
}

fn collect_output_base(cmd: &mut Command) -> Result<(ExitStatus, String, String), GitError> {
    match cmd.output() {
        Err(err) => match err.kind() {
            ErrorKind::NotFound => Err(GitError::NotFound),
            _ => Err(GitError::OtherError(err.into())),
        },
        Ok(result) => Ok((
            result.status,
            String::from_utf8(result.stdout).map_err(|e| GitError::OtherError(e.into()))?,
            String::from_utf8(result.stderr).map_err(|e| GitError::OtherError(e.into()))?,
        )),
    }
}

pub fn check_version() -> Result<(), GitError> {
    let (status, stdout, stderr) =
        collect_output_base(Command::new("git").stdin(Stdio::null()).arg("--version"))?;
    if status.success() {
        for line in stdout.lines() {
            match strip_prefix("git version ", line) {
                None => {}
                Some(version_str) => {
                    let version = Version::parse(&version_str)
                        .context("Error parsing git version")
                        .map_err(|e| GitError::OtherError(e.into()))?;
                    if version < MIN_GIT_VERSION {
                        return Err(GitError::TooOld(version));
                    }
                    return Ok(());
                }
            }
        }
        Err(GitError::OtherError(format_err!(
            "Couldn't find version in the output of {}. Output was:\n{}",
            color_cmd(&"git --version"),
            stdout
        )))
    } else {
        Err(GitError::ExitStatus(
            "git --version".to_string(),
            stderr,
            status,
        ))
    }
}

pub fn warn_env() {
    warn_env_var("GIT_DIR");
    warn_env_var("GIT_WORK_TREE");
}

fn warn_env_var(var_name: &str) {
    match env::var(var_name) {
        Err(env::VarError::NotPresent) => (),
        Err(env::VarError::NotUnicode(_)) => println!(
            "{} {} environment is set to a non-unicode string,\n         \"
             and will be used with mzr's git invocations.",
            color_warn(&"Warning:"),
            var_name,
        ),
        Ok(v) => println!(
            "{} {} environment variable is set to {},\n         \
             and will be used with mzr's git invocations.",
            color_warn(&"Warning:"),
            var_name,
            color_dir(&v)
        ),
    }
}

/*
 * Git errors
 */

#[derive(Debug, Fail)]
pub enum GitError {
    NotFound,
    TooOld(Version),
    ExitStatus(String, String, ExitStatus),
    OtherError(Error),
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::NotFound => write!(f, "'git' not found on your PATH environment variable."),
            GitError::TooOld(v) => write!(
                f,
                "You have git version {}, but mzr requires at least version {}",
                v, MIN_GIT_VERSION
            ),
            GitError::ExitStatus(cmd, _, status) => write!(
                f,
                "{} exited with error status {}",
                color_cmd(cmd),
                color_err(status)
            ),
            GitError::OtherError(err) => err.fmt(f),
        }
    }
}

// Minimum git version currently based on usage of "git symbolic-ref" command.
const MIN_GIT_VERSION: Version = Version {
    major: 1,
    minor: 8,
    patch: 0,
    pre: Vec::new(),
    build: Vec::new(),
};
