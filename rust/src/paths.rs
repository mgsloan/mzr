use colors::*;
use std::convert::AsRef;
use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};
use utils::add_suffix_to_path;

/// Path to the mizer directory - typically something like
/// `.../PROJECT.mizer`, a sibling of `.../PROJECT`.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct MizerDir(PathBuf);

/// Path to the user's work directory. This is the "target" path of the
/// overlayfs mount.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct UserWorkDir(PathBuf);

/// Project dir where the user invoked mizer.
///
/// TODO(snapshots): In the near future this will probably be the same as
/// `UserWorkDir`.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct UserRootDir(PathBuf);

/// Path to the zone directory within the mizer directory - typically something
/// like `.../PROJECT.mizer/zone/ZONE`.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct ZoneDir(PathBuf);

/// Name of a zone.
///
/// TODO(zone-name-validation): document validation once it has that.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct ZoneName(String);

/// Path to snapshot directory.
///
/// TODO(snapshots): document typical directory once it is controlled.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct SnapDir(PathBuf);

/// Path to the zone changes directory - typically something like
/// `.../PROJECT.mizer/zone/ZONE/changes`. This is used as the "upper" dir of
/// the overlayfs mount, and so changes that overlay the snapshot are stored
/// here, hence the name `changes`.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct ChangesDir(PathBuf);

/// Path to the overlayfs work directory. This must be in the same filesystem as
/// the associated `ChangesDir`, because it is used to prepare files before
/// putting them in the upper dir.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct OvfsWorkDir(PathBuf);

impl MizerDir {
    pub fn new(root_dir: &UserRootDir) -> MizerDir {
        MizerDir(add_suffix_to_path(root_dir, ".mizer"))
    }
}

impl UserWorkDir {
    pub fn new(root_dir: &UserRootDir) -> UserWorkDir {
        UserWorkDir(add_suffix_to_path(root_dir, ".work"))
    }
}

impl UserRootDir {
    pub fn new(root_dir: &PathBuf) -> UserRootDir {
        UserRootDir(root_dir.clone())
    }
}

impl ZoneDir {
    pub fn new(mizer_dir: &MizerDir, zone_name: &ZoneName) -> ZoneDir {
        let mizer_dir_buf: &PathBuf = mizer_dir.as_ref();
        let mut zone_dir = mizer_dir_buf.clone();
        zone_dir.push("zone");
        zone_dir.push(zone_name);
        ZoneDir(zone_dir)
    }
}

impl ZoneName {
    // TODO(zone-name-validation)
    pub fn new(name: String) -> ZoneName {
        ZoneName(name)
    }
}

impl SnapDir {
    // TODO(snapshots): for now, the root dir is used as the lower dir.
    pub fn new(root_dir: &UserRootDir) -> SnapDir {
        SnapDir(root_dir.0.clone())
    }
}

impl ChangesDir {
    pub fn new(zone_dir: &ZoneDir) -> ChangesDir {
        let mut changes_dir = zone_dir.0.clone();
        changes_dir.push("changes");
        ChangesDir(changes_dir)
    }
}

impl OvfsWorkDir {
    pub fn new(zone_dir: &ZoneDir) -> OvfsWorkDir {
        let mut ovfs_work_dir = zone_dir.0.clone();
        ovfs_work_dir.push("ovfs-work");
        OvfsWorkDir(ovfs_work_dir)
    }
}

impl AsRef<Path> for MizerDir {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for UserWorkDir {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for UserRootDir {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for ZoneDir {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for ZoneName {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for SnapDir {
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

impl Display for MizerDir {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        color_dir(&self.0.display()).fmt(f)
    }
}

impl Display for UserWorkDir {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        color_dir(&self.0.display()).fmt(f)
    }
}

impl Display for UserRootDir {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        color_dir(&self.0.display()).fmt(f)
    }
}

impl Display for ZoneDir {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        color_dir(&self.0.display()).fmt(f)
    }
}

impl Display for ZoneName {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        color_zone_name(&self.0).fmt(f)
    }
}

impl Display for SnapDir {
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
