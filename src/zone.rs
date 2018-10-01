use chrono::{DateTime, Utc};
use crate::colors::color_dir;
use crate::json;
use crate::paths::*;
use failure::{Error, ResultExt};
use libmount::{BindMount, Overlay};
use serde_derive::{Deserialize, Serialize};
use std::fs::{create_dir, create_dir_all};
use std::iter;

#[derive(Debug)]
pub struct Zone {
    pub name: ZoneName,
    pub zone_dir: ZoneDir,
    pub snap_dir: SnapDir,
    pub ovfs_changes_dir: OvfsChangesDir,
    pub ovfs_work_dir: OvfsWorkDir,
    pub ovfs_mount_dir: OvfsMountDir,
    pub info: ZoneInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ZoneInfo {
    pub snapshot: SnapName,
    pub creation_time: DateTime<Utc>,
}

impl Zone {
    pub fn create(
        mzr_dir: &MzrDir,
        zone_name: &ZoneName,
        snap_name: &SnapName,
    ) -> Result<Zone, Error> {
        let zone_dir = ZoneDir::new(mzr_dir, &zone_name);
        Zone::create_impl(mzr_dir, &zone_dir, zone_name, snap_name)
    }

    pub fn load(mzr_dir: &MzrDir, zone_name: &ZoneName) -> Result<Zone, Error> {
        let zone_dir = ZoneDir::new(mzr_dir, &zone_name);
        Zone::load_impl(mzr_dir, &zone_dir, &zone_name)
    }

    pub fn load_if_exists(mzr_dir: &MzrDir, zone_name: &ZoneName) -> Result<Option<Zone>, Error> {
        let zone_dir = ZoneDir::new(mzr_dir, &zone_name);
        if zone_dir.is_dir() {
            Ok(Some(Zone::load_impl(mzr_dir, &zone_dir, &zone_name)?))
        } else {
            Ok(None)
        }
    }

    pub fn exists(mzr_dir: &MzrDir, zone_name: &ZoneName) -> bool {
        ZoneDir::new(mzr_dir, &zone_name).is_dir()
    }

    pub fn load_or_create<F>(
        mzr_dir: &MzrDir,
        zone_name: &ZoneName,
        get_snap_name: F,
    ) -> Result<Zone, Error>
    where
        F: FnOnce() -> Result<SnapName, Error>,
    {
        let zone_dir = ZoneDir::new(mzr_dir, &zone_name);
        if zone_dir.is_dir() {
            Zone::load_impl(mzr_dir, &zone_dir, &zone_name)
        } else {
            let snap_name = get_snap_name()?;
            Zone::create_impl(mzr_dir, &zone_dir, zone_name, &snap_name)
        }
    }

    fn create_impl(
        mzr_dir: &MzrDir,
        zone_dir: &ZoneDir,
        zone_name: &ZoneName,
        snap_name: &SnapName,
    ) -> Result<Zone, Error> {
        let snap_dir = SnapDir::new(mzr_dir, &snap_name);
        if !snap_dir.is_dir() {
            bail!(
                "Expected that the {} snapshot would exist at {}",
                snap_name,
                snap_dir
            );
        }
        let zone_parent = zone_dir
            .parent()
            .ok_or_else(|| format_err!("Unexpected error: zone directory must have a parent."))?;
        create_dir_all(zone_parent).context(format_err!(
            "Unexpected error while creating zone parent directory {}",
            color_dir(&zone_parent.display())
        ))?;
        match create_dir(zone_dir.clone()) {
            Err(e) => {
                if zone_dir.exists() {
                    Err(e).context(format_err!(
                        "{} zone already exists at {}",
                        zone_name,
                        zone_dir
                    ))?
                } else {
                    Err(e).context(format_err!(
                        "Unexpected error while creating zone directory {}",
                        zone_dir
                    ))?
                }
            }
            Ok(()) => {
                let ovfs_changes_dir = OvfsChangesDir::new(&zone_dir);
                let ovfs_work_dir = OvfsWorkDir::new(&zone_dir);
                let ovfs_mount_dir = OvfsMountDir::new(&zone_dir);
                create_dir_all(ovfs_changes_dir.clone()).context(format_err!(
                    "Unexpected error while creating zone changes directory for overlayfs: {}",
                    ovfs_changes_dir
                ))?;
                create_dir_all(ovfs_work_dir.clone()).context(format_err!(
                    "Unexpected error while creating zone work directory for overlayfs: {}",
                    ovfs_work_dir
                ))?;
                create_dir_all(ovfs_mount_dir.clone()).context(format_err!(
                    "Unexpected error while creating zone mount directory for overlayfs: {}",
                    ovfs_mount_dir
                ))?;
                let info = ZoneInfo {
                    snapshot: snap_name.clone(),
                    creation_time: Utc::now(),
                };
                json::write(&ZoneInfoFile::new(&zone_dir), &info)?;
                Ok(Zone {
                    name: zone_name.clone(),
                    zone_dir: zone_dir.clone(),
                    snap_dir,
                    ovfs_changes_dir,
                    ovfs_work_dir,
                    ovfs_mount_dir,
                    info,
                })
            }
        }
    }

    pub fn load_impl(
        mzr_dir: &MzrDir,
        zone_dir: &ZoneDir,
        zone_name: &ZoneName,
    ) -> Result<Zone, Error> {
        let info: ZoneInfo = json::read(&ZoneInfoFile::new(&zone_dir))?.contents;
        let snap_dir = SnapDir::new(mzr_dir, &info.snapshot);
        let ovfs_changes_dir = OvfsChangesDir::new(zone_dir);
        let ovfs_work_dir = OvfsWorkDir::new(zone_dir);
        let ovfs_mount_dir = OvfsMountDir::new(zone_dir);
        Ok(Zone {
            name: zone_name.clone(),
            zone_dir: zone_dir.clone(),
            snap_dir,
            ovfs_changes_dir,
            ovfs_work_dir,
            ovfs_mount_dir,
            info,
        })
    }

    pub fn mount(&self) -> Result<(), Error> {
        Overlay::writable(
            iter::once(self.snap_dir.as_ref()),
            &self.ovfs_changes_dir,
            &self.ovfs_work_dir,
            &self.ovfs_mount_dir,
        ).mount()
        // TODO(cleanup): Should make it so that '?' can be used,
        // by making libmount Error implement Sync. Same pattern
        // repeated below for bind mount.
        .map_err(|e| format_err!("{}", e))
    }

    pub fn bind_to(&self, user_work_dir: &UserWorkDir) -> Result<(), Error> {
        BindMount::new(&self.ovfs_mount_dir, &user_work_dir)
            .mount()
            .map_err(|e| format_err!("{}", e))
    }
}
