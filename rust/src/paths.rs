use colors::*;
use failure::Error;
use std::convert::AsRef;
use std::ffi::OsStr;
use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use utils::add_suffix_to_path;

/// Path to the mzr directory - typically something like `.../PROJECT.mzr`, a
/// sibling of `.../PROJECT`.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct MzrDir(PathBuf);

/// Path to the user's work directory. This is the "target" path of the
/// overlayfs mount.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct UserWorkDir(PathBuf);

/// Path to the zone directory within the mzr directory - typically something
/// like `.../PROJECT.mzr/zone/ZONE`.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct ZoneDir(PathBuf);

/// Path to snapshot directory- typically something like
/// `.../PROJECT.mzr/snap/SNAP`.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct SnapDir(PathBuf);

/// Path to a temporary location for the snapshot directory - typically
/// something like `.../PROJECT.mzr/snap-tmp/SNAP`. This is used for in-progress
/// snapshots. This way, `SnapDir` should always contain fully frozen and
/// complete snapshots.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct SnapTmpDir(PathBuf);

/// Path to the zone changes directory - typically something like
/// `.../PROJECT.mzr/zone/ZONE/changes`. This is used as the "upper" dir of the
/// overlayfs mount, and so changes that overlay the snapshot are stored here,
/// hence the name `changes`.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct ChangesDir(PathBuf);

/// Path to the overlayfs work directory. This must be in the same filesystem as
/// the associated `ChangesDir`, because it is used to prepare files before
/// putting them in the upper dir.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct OvfsWorkDir(PathBuf);

/// Name of a zone.
///
/// TODO(name-validation): document validation once it has that.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct ZoneName(String);

/// Name of a zone.
///
/// TODO(name-validation): document validation once it has that.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct SnapName(String);

impl MzrDir {
    pub fn new(work_dir: &UserWorkDir) -> Self {
        MzrDir(add_suffix_to_path(work_dir, ".mzr"))
    }
}

impl UserWorkDir {
    pub fn new(work_dir: &PathBuf) -> Self {
        UserWorkDir(work_dir.clone())
    }

    pub fn to_arg(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl ZoneDir {
    pub fn new(mzr_dir: &MzrDir, zone_name: &ZoneName) -> Self {
        let mzr_dir_buf: &PathBuf = mzr_dir.as_ref();
        let mut result = mzr_dir_buf.clone();
        result.push("zone");
        result.push(zone_name);
        ZoneDir(result)
    }
}

impl SnapDir {
    pub fn new(mzr_dir: &MzrDir, snap_name: &SnapName) -> Self {
        let mzr_dir_buf: &PathBuf = mzr_dir.as_ref();
        let mut result = mzr_dir_buf.clone();
        result.push("snap");
        result.push(snap_name);
        SnapDir(result)
    }

    pub fn to_arg(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl SnapTmpDir {
    // TODO(snapshots): for now, the root dir is used as the lower dir.
    pub fn new(mzr_dir: &MzrDir, snap_name: &SnapName) -> Self {
        let mzr_dir_buf: &PathBuf = mzr_dir.as_ref();
        let mut result = mzr_dir_buf.clone();
        result.push("snap-tmp");
        result.push(snap_name);
        SnapTmpDir(result)
    }

    pub fn to_arg(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl ChangesDir {
    pub fn new(zone_dir: &ZoneDir) -> Self {
        let mut changes_dir = zone_dir.0.clone();
        changes_dir.push("changes");
        ChangesDir(changes_dir)
    }
}

impl OvfsWorkDir {
    pub fn new(zone_dir: &ZoneDir) -> Self {
        let mut ovfs_work_dir = zone_dir.0.clone();
        ovfs_work_dir.push("ovfs-work");
        OvfsWorkDir(ovfs_work_dir)
    }
}

impl ZoneName {
    pub fn new(name: String) -> Result<Self, Error> {
        // TODO(name-validation)
        Ok(ZoneName(name))
    }
}

impl FromStr for ZoneName {
    type Err = Error;
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Ok(ZoneName::new(name.to_string())?)
    }
}

impl SnapName {
    pub fn new(name: String) -> Result<Self, Error> {
        // TODO(name-validation)
        Ok(SnapName(name))
    }
}

impl FromStr for SnapName {
    type Err = Error;
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Ok(SnapName::new(name.to_string())?)
    }
}

impl AsRef<Path> for MzrDir {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for UserWorkDir {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for ZoneDir {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for SnapDir {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for SnapTmpDir {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for ChangesDir {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for OvfsWorkDir {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for ZoneName {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for SnapName {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl Display for MzrDir {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        color_dir(&self.0.display()).fmt(f)
    }
}

impl Display for UserWorkDir {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        color_dir(&self.0.display()).fmt(f)
    }
}

impl Display for ZoneDir {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        color_dir(&self.0.display()).fmt(f)
    }
}

impl Display for SnapDir {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        color_dir(&self.0.display()).fmt(f)
    }
}

impl Display for SnapTmpDir {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        color_dir(&self.0.display()).fmt(f)
    }
}

impl Display for ChangesDir {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        color_dir(&self.0.display()).fmt(f)
    }
}

impl Display for OvfsWorkDir {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        color_dir(&self.0.display()).fmt(f)
    }
}

impl Display for ZoneName {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        color_zone_name(&self.0).fmt(f)
    }
}

impl Display for SnapName {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        color_snap_name(&self.0).fmt(f)
    }
}
