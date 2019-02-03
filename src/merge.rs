use crate::colors::*;
use crate::paths::OvfsChangesDir;
use crate::utils::run_process;
use crate::zone::Zone;
use failure::Error;
use std::fs;
use std::fs::Metadata;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use walkdir::WalkDir;

pub enum Mode {
    AlwaysAsk,
    AutoApplyUpdates,
    AutoApplyConflicts,
}

pub fn interactive_merge(zone: &Zone, target_dir: &PathBuf, mode: Mode) -> Result<(), Error> {
    let plan = plan_merging_zone_changes(zone, &target_dir);
    if plan.skips.len() > 0 {
        println!("Skipping merging the following paths:");
        for skip in plan.skips {
            // TODO(cleanliness): use option combinator
            match skip.source {
                None => println!("* <missing>"),
                Some(path) => println!("* {:?}", path),
            }
        }
    }

    // TODO(next-steps): Thinking that the best way to do this would be to not have an interactive
    // mode. Instead, have an editable file, similar to what is used for rebase.

    /*
    // TODO(cleanliness): There must be a function testing emptiness
    let update_count = plan.updates.len();
    let conflict_count = plan.conflicts.len();
    match mode {
        AutoApplyUpdates => {
            for update in plan.updates {
                update.apply(&zone.ovfs_changes_dir, &target_dir)?;
            }
        }
        AutoApplyConflicts => {
            for update in plan.updates {
                update.apply(&zone.ovfs_changes_dir, &target_dir)?;
            }
        }
    */

    /*
    if update_count > 0 || conflict_count > 0 {
        match (mode, has_updates, has_conflicts) {
            (Mode::AutoApplyUpdates, _, false) => {
                apply_updates();
                println!("Updated {} file(s)", color_success(plan.updates.len()));
            }
            (Mode::AutoApplyConflicts, _, _) => {
                for update in plan.updates {
                    update.apply(&zone.ovfs_changes_dir, &target_dir)?;
                }
                for conflict in plan.conflicts {
                    conflict.apply(&zone.ovfs_changes_dir, &target_dir)?;
                }
                println!(
                    "Updated {} file(s), where {} were overwrites of conflicting file(s).",
                    color_success(update_count + conflict_count),
                    color_Warn(conflict_count)
                );
            }
            _ => {}
        }
    } else {
        println!(
            "{} No changes to merge.",
            color_success(&String::from("Success: "))
        );
    }
    */
    Ok(())
}

pub struct Plan {
    pub updates: Vec<Update>,
    pub conflicts: Vec<Conflict>,
    pub skips: Vec<Skip>,
}

pub struct Update {
    pub rel_path: PathBuf,
    pub source_metadata: Metadata,
    pub target_metadata: Option<Metadata>,
}

pub struct Conflict {
    pub rel_path: PathBuf,
    pub reason: ConflictReason,
    pub source_metadata: Metadata,
    pub target_metadata: Metadata,
}

pub enum ConflictReason {
    NotInSnapshot,
    ModifiedInTarget,
}

pub struct Skip {
    pub source: Option<PathBuf>,
    pub reason: Error,
}

impl Update {
    fn apply(&self, changes_dir: &OvfsChangesDir, target_dir: &PathBuf) -> Result<(), Error> {
        copy_from_changes_dir(&self.rel_path, changes_dir, target_dir)
    }
}

impl Conflict {
    fn apply(&self, changes_dir: &OvfsChangesDir, target_dir: &PathBuf) -> Result<(), Error> {
        copy_from_changes_dir(&self.rel_path, changes_dir, target_dir)
    }
}

// TODO(correctness): Check expected metadata
fn copy_from_changes_dir(
    rel_path: &PathBuf,
    changes_dir: &OvfsChangesDir,
    target_dir: &PathBuf,
) -> Result<(), Error> {
    let source = changes_dir.join(rel_path.clone());
    let target = target_dir.join(rel_path.clone());
    copy_file(&source, &target)
}

/// Copies a file from source path to target path, using cp in order to support reflinks.
fn copy_file(source: &PathBuf, target: &PathBuf) -> Result<(), Error> {
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
        // Don't dereference source symlinks.
        .arg("--no-dereference")
        .arg(source)
        .arg(target);
    run_process(cmd)
}

/// This enumerates every file in change directory of `zone`, and creates a `Plan` for applying
/// those changes to the specified `target_dir`.
///
/// This plan will turn these changed files into updates if the file has not been changed in the
/// target dir. Whether the file has been changed in the target dir is determined by comparing its
/// metadata to the metadata of the corresponding file in the snapshot.
fn plan_merging_zone_changes(zone: &Zone, target_dir: &PathBuf) -> Plan {
    let source_dir = zone.ovfs_changes_dir.clone();
    let mut updates = Vec::new();
    let mut conflicts = Vec::new();
    let mut skips = Vec::new();
    for walk_result in WalkDir::new(&source_dir).same_file_system(true) {
        match walk_result {
            Err(e) => skips.push(Skip {
                source: e.path().map(PathBuf::from),
                reason: Error::from(e),
            }),
            Ok(entry) => {
                let source = PathBuf::from(entry.path());
                let result: Result<(), Error> = try {
                    let source_metadata = entry.metadata()?;
                    // For now, emulating git's precedent of ignoring dirs.
                    if !source_metadata.is_dir() {
                        let rel_path = PathBuf::from(source.strip_prefix(&source_dir)?);
                        let target = target_dir.join(&rel_path);
                        match get_metadata(&target)? {
                            None => updates.push(Update {
                                rel_path,
                                source_metadata,
                                target_metadata: None,
                            }),
                            Some(target_metadata) => {
                                // Note that this relies on snapshotting preserving timestamps.
                                let snapshot = zone.snap_dir.join(&rel_path);
                                match get_metadata(&snapshot)? {
                                    // The file didn't exist in the snapshot, but now exists in both
                                    // working dirs, so it's a conflict.
                                    None => conflicts.push(Conflict {
                                        rel_path,
                                        reason: ConflictReason::NotInSnapshot,
                                        source_metadata,
                                        target_metadata,
                                    }),
                                    Some(snapshot_metadata) => {
                                        if metadata_matches(&target_metadata, &snapshot_metadata) {
                                            updates.push(Update {
                                                rel_path,
                                                source_metadata,
                                                target_metadata: Some(target_metadata),
                                            });
                                        } else {
                                            conflicts.push(Conflict {
                                                rel_path,
                                                reason: ConflictReason::ModifiedInTarget,
                                                source_metadata,
                                                target_metadata,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                };
                result.err().map(|reason| {
                    skips.push(Skip {
                        source: Some(source),
                        reason,
                    })
                });
            }
        }
    }
    Plan {
        updates,
        conflicts,
        skips,
    }
}

fn get_metadata(path: &PathBuf) -> Result<Option<Metadata>, Error> {
    // Note that this function gets metadata without looking through symlinks.  We really don't want
    // to try to look through symlinks, since relative symlinks won't resolve correctly anyway.
    match fs::symlink_metadata(path) {
        Err(e) => match e.kind() {
            ErrorKind::NotFound => Ok(None),
            _ => Err(Error::from(e)),
        },
        Ok(metadata) => Ok(Some(metadata)),
    }
}

fn metadata_matches(x: &Metadata, y: &Metadata) -> bool {
    // Check things that are most likely to differ first.
    if x.len() != y.len() {
        return false;
    }
    match (x.modified(), y.modified()) {
        (Ok(x_time), Ok(y_time)) => if x_time != y_time {
            return false;
        },
        // TODO(correctness): Can this ever happen? I don't think so.
        _ => return false,
    }
    if x.permissions() != y.permissions() {
        return false;
    }
    // Highly unlikely that these would differ, but may as well check.
    let x_type = x.file_type();
    let y_type = y.file_type();
    x_type.is_dir() == y_type.is_dir()
        && x_type.is_file() == y_type.is_file()
        && x_type.is_symlink() == y_type.is_symlink()
}
