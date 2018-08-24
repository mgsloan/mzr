use colors::*;
use failure::{Error, ResultExt};
use paths::*;
use std::fs::{create_dir, create_dir_all, remove_dir, remove_dir_all, rename};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use top_dirs::TopDirs;
use utils::{confirm, Confirmed};

pub fn of_workdir(top_dirs: &TopDirs, snap_name: &SnapName) -> Result<SnapDir, Error> {
    create(&top_dirs.user_work_dir, &top_dirs.mzr_dir, snap_name)
}

fn create(source_dir: &PathBuf, mzr_dir: &MzrDir, snap_name: &SnapName) -> Result<SnapDir, Error> {
    let snap_dir = &SnapDir::new(mzr_dir, snap_name);
    if snap_dir.exists() {
        // TODO(friendliness): Should suggest "mzr rm" feature once it exists.
        bail!("A snapshot named {} already exists.", snap_name);
    }
    let snap_tmp_dir = &ensure_tmp_dir(mzr_dir, &snap_name)?;
    let mut cmd_base = Command::new("cp");
    let cmd = cmd_base
        .stdin(Stdio::null())
        // Preserve all file properties, and preserve symlinks.
        .arg("--archive")
        // When using filesystems that support reflinks, use them. Filesystems
        // like BTRFS and XFS support creating copy-on-write copies of files.
        // When using reflinks to make a snapshot, it's pretty comparable to
        // creating a tree of hardlinks, which tends to be much faster.
        .arg("--reflink=auto")
        // Don't clobber files. Shouldn't happen, since we check for destination
        // of the target. But if it does happen, then something funky is
        // happening and we should exit.
        .arg("--no-clobber")
        // While `ensure_tmp_dir` checked if the directory already exists, it is
        // possible for that to change between the check and the cp invocation.
        // This makes it so that `cp` doesn't use its default behavior of
        // copying into the target directory if the destination is a directory.
        .arg("--no-target-directory")
        // Source directory
        .arg(source_dir)
        .arg(snap_tmp_dir.to_arg());
    println!("Taking a snapshot named {}", snap_name);
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
    rename(snap_tmp_dir, snap_dir).context(format_err!(
        "Unexpected error moving the temporary directory for {} into the snapshots folder", snap_name
    ))?;
    // Remove snap-tmp directory, if possible. Ignore error results, since it's
    // anticipated that sometimes the tmp dir may have other contents.
    let _ = snap_tmp_dir.parent().map(|x| remove_dir(x));
    // Let the user know that it succeeded.
    println!("Finished taking snapshot.");
    // TODO(cleanup): Can this clone be avoided?
    Ok(snap_dir.clone())
}

/// Ensures that the `SnapTmpDir` doesn't exist, and also ensures that its
/// parent directories do exist.
///
/// If it does exist, then prompts the user to attempt to delete it.
fn ensure_tmp_dir(mzr_dir: &MzrDir, snap_name: &SnapName) -> Result<SnapTmpDir, Error> {
    let snap_tmp_dir = &SnapTmpDir::new(mzr_dir, &snap_name);
    // TODO(friendliness): Would be nice to detect if a concurrent mzr is
    // actively snapshotting.
    if snap_tmp_dir.exists() {
        println!(
            "Temporary directory to use as copy target already exists at {}",
            snap_tmp_dir
        );
        match confirm(&"Attempt to delete this directory")? {
            Confirmed::Yes => remove_dir_all(snap_tmp_dir)?,
            Confirmed::No => bail!("Aborting "),
        };
    }
    snap_tmp_dir.parent().map(|x| create_dir_all(x));
    // TODO(cleanup): Can this clone be avoided?
    Ok(snap_tmp_dir.clone())
}
