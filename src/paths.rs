use crate::colors::*;
use crate::utils::add_suffix_to_path;
use failure::Error;
use nix::libc::pid_t;
use nix::unistd::Pid;
use serde_derive::{Deserialize, Serialize};
use shrinkwraprs::Shrinkwrap;
use std::convert::AsRef;
use std::ffi::OsStr;
use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};
use std::str::FromStr;

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

/// Path to the zone info file - typically something
/// like `.../PROJECT.mzr/zone/ZONE/info.json`.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct ZoneInfoFile(PathBuf);

/// Path to snapshot directory - typically something like
/// `.../PROJECT.mzr/snap/SNAP`.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct SnapDir(PathBuf);

/// Path to the zone changes directory - typically something like
/// `.../PROJECT.mzr/zone/ZONE/changes`. This is used as the "upper"
/// dir of the overlayfs mount, and so changes that overlay the
/// snapshot are stored here, hence the name `changes`.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct OvfsChangesDir(PathBuf);

/// Path to the overlayfs work directory. This must be in the same filesystem as
/// the associated `OvfsChangesDir`, because it is used to prepare files before
/// putting them in the upper dir.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct OvfsWorkDir(PathBuf);

/// Path to the zone mount directory - typically something like
/// `.../PROJECT.mzr/zone/ZONE/mount`. This is used as the mount
/// target for the overlayfs mount.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct OvfsMountDir(PathBuf);

/// Path to the directory containing daemon related files. It is
/// typically something like `.../PROJECT.mzr/daemon`.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct DaemonDir(PathBuf);

/// Path to the daemon pid-file, which stores the process id of the
/// mzr daemon. It is typically something like
/// `.../PROJECT.mzr/daemon/process`.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct DaemonPidFile(PathBuf);

/// Path to the daemon log file - typically something like
/// `.../PROJECT.mzr/daemon/log`.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct DaemonLogFile(PathBuf);

/// Path to the daemon unix domain socket - typically something like
/// `.../PROJECT.mzr/daemon/socket`.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct DaemonSocketFile(PathBuf);

/// Path for a process, within the proc filesystem - typically
/// something like `/proc/PID`, where `PID` is the process identifier
/// of a running process.
#[derive(Debug, Clone, Shrinkwrap)]
pub struct ProcDir(PathBuf);

/// Path to a proc filesystem namespace file, such as
/// `/proc/PID/ns/mount` or `/proc/PID/ns/user`.
pub struct ProcNamespaceFile(PathBuf);

/// Name of a zone.
///
/// TODO(name-validation): document validation once it has that.
#[derive(Debug, Clone, Shrinkwrap, Serialize, Deserialize)]
pub struct ZoneName(String);

/// Name of a snapshot.
///
/// TODO(name-validation): document validation once it has that.
#[derive(Debug, Clone, Shrinkwrap, Serialize, Deserialize)]
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

    #[allow(dead_code)]
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

impl ZoneInfoFile {
    pub fn new(zone_dir: &ZoneDir) -> Self {
        let zone_info_buf: &PathBuf = zone_dir.as_ref();
        let mut result = zone_info_buf.clone();
        result.push("info.json");
        ZoneInfoFile(result)
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

impl OvfsChangesDir {
    pub fn new(zone_dir: &ZoneDir) -> Self {
        let mut ovfs_changes_dir = zone_dir.0.clone();
        ovfs_changes_dir.push("changes");
        OvfsChangesDir(ovfs_changes_dir)
    }
}

impl OvfsWorkDir {
    pub fn new(zone_dir: &ZoneDir) -> Self {
        let mut ovfs_work_dir = zone_dir.0.clone();
        ovfs_work_dir.push("ovfs-work");
        OvfsWorkDir(ovfs_work_dir)
    }
}

impl OvfsMountDir {
    pub fn new(zone_dir: &ZoneDir) -> Self {
        let mut ovfs_mount_dir = zone_dir.0.clone();
        ovfs_mount_dir.push("mount");
        OvfsMountDir(ovfs_mount_dir)
    }
}

impl DaemonDir {
    pub fn new(mzr_dir: &MzrDir) -> Self {
        let mzr_dir_buf: &PathBuf = mzr_dir.as_ref();
        let mut result = mzr_dir_buf.clone();
        result.push("daemon");
        DaemonDir(result)
    }
}

impl DaemonPidFile {
    pub fn new(daemon_dir: &DaemonDir) -> Self {
        let dir_buf: &PathBuf = daemon_dir.as_ref();
        let mut result = dir_buf.clone();
        result.push("process.pid");
        DaemonPidFile(result)
    }
}

impl DaemonLogFile {
    pub fn new(daemon_dir: &DaemonDir) -> Self {
        let dir_buf: &PathBuf = daemon_dir.as_ref();
        let mut result = dir_buf.clone();
        result.push("log");
        DaemonLogFile(result)
    }
}

impl DaemonSocketFile {
    pub fn new(daemon_dir: &DaemonDir) -> Self {
        let dir_buf: &PathBuf = daemon_dir.as_ref();
        let mut result = dir_buf.clone();
        result.push("socket");
        DaemonSocketFile(result)
    }
}

impl ProcDir {
    pub fn new(pid: Pid) -> Self {
        let mut dir_buf = PathBuf::from("/proc");
        dir_buf.push(pid_t::from(pid).to_string());
        ProcDir(dir_buf)
    }
}

impl ProcNamespaceFile {
    pub fn new_mount(dir: &ProcDir) -> Self {
        Self::new(dir, "mnt")
    }

    pub fn new_user(dir: &ProcDir) -> Self {
        Self::new(dir, "user")
    }

    fn new<P: AsRef<Path>>(dir: &ProcDir, subdir: P) -> Self {
        let dir_buf: &PathBuf = dir.as_ref();
        let mut result = dir_buf.clone();
        result.push("ns");
        result.push(subdir);
        ProcNamespaceFile(result)
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

impl AsRef<Path> for ZoneInfoFile {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for SnapDir {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for OvfsChangesDir {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for OvfsWorkDir {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for OvfsMountDir {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for DaemonDir {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for DaemonPidFile {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for DaemonLogFile {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for DaemonSocketFile {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for ProcDir {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Path> for ProcNamespaceFile {
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

impl AsRef<OsStr> for MzrDir {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl AsRef<OsStr> for UserWorkDir {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl AsRef<OsStr> for ZoneDir {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl AsRef<OsStr> for ZoneInfoFile {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl AsRef<OsStr> for SnapDir {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl AsRef<OsStr> for OvfsChangesDir {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl AsRef<OsStr> for OvfsWorkDir {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl AsRef<OsStr> for OvfsMountDir {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl AsRef<OsStr> for DaemonDir {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl AsRef<OsStr> for DaemonPidFile {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl AsRef<OsStr> for DaemonLogFile {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl AsRef<OsStr> for DaemonSocketFile {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl AsRef<OsStr> for ProcDir {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl AsRef<OsStr> for ProcNamespaceFile {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl AsRef<OsStr> for ZoneName {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl AsRef<OsStr> for SnapName {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl Display for MzrDir {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        color_dir(&self.0.display()).fmt(f)
    }
}

impl Display for UserWorkDir {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        color_dir(&self.0.display()).fmt(f)
    }
}

impl Display for ZoneDir {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        color_dir(&self.0.display()).fmt(f)
    }
}

impl Display for ZoneInfoFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        color_file(&self.0.display()).fmt(f)
    }
}

impl Display for SnapDir {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        color_dir(&self.0.display()).fmt(f)
    }
}

impl Display for OvfsChangesDir {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        color_dir(&self.0.display()).fmt(f)
    }
}

impl Display for OvfsWorkDir {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        color_dir(&self.0.display()).fmt(f)
    }
}

impl Display for OvfsMountDir {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        color_dir(&self.0.display()).fmt(f)
    }
}

impl Display for DaemonDir {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        color_file(&self.0.display()).fmt(f)
    }
}

impl Display for DaemonPidFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        color_file(&self.0.display()).fmt(f)
    }
}

impl Display for DaemonLogFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        color_file(&self.0.display()).fmt(f)
    }
}

impl Display for DaemonSocketFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        color_file(&self.0.display()).fmt(f)
    }
}

impl Display for ProcDir {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        color_dir(&self.0.display()).fmt(f)
    }
}

impl Display for ProcNamespaceFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        color_file(&self.0.display()).fmt(f)
    }
}

impl Display for ZoneName {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        color_zone_name(&self.0).fmt(f)
    }
}

impl Display for SnapName {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        color_snap_name(&self.0).fmt(f)
    }
}
