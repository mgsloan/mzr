use colors::*;
use failure::{Error, ResultExt};
use paths::*;
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use top_dirs::TopDirs;

pub fn of_workdir(top_dirs: &TopDirs, snap_name: &SnapName) -> Result<SnapDir, Error> {
    create(&top_dirs.user_work_dir, &top_dirs.mzr_dir, snap_name)
}

fn create(source_dir: &PathBuf, mzr_dir: &MzrDir, snap_name: &SnapName) -> Result<SnapDir, Error> {
    let snap_dir = &SnapDir::new(mzr_dir, snap_name);
    if snap_dir.exists() {
        // TODO(friendliness): Should suggest "mzr rm" feature once it exists.
        bail!("A snapshot named {} already exists.", snap_name);
    }
    let snap_parent = snap_dir.parent().ok_or(format_err!(
        "Unexpected error: snapshot directory must have a parent."
    ))?;
    create_dir_all(snap_parent).context(format_err!(
        "Unexpected error while creating snapshot parent directory {}",
        color_dir(&snap_parent.display())
    ))?;
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
        .arg(snap_dir.to_arg());
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
    // TODO(cleanup): Can this clone be avoided?
    Ok(snap_dir.clone())
}
