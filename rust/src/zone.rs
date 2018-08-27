use chrono::{DateTime, Utc};
use failure::{Error, ResultExt};
use libmount::Overlay;
use std::fs::{create_dir, create_dir_all};
use std::iter;

use colors::color_dir;
use json;
use paths::*;

#[derive(Debug)]
pub struct Zone {
    pub zone_dir: ZoneDir,
    pub snap_dir: SnapDir,
    pub changes_dir: ChangesDir,
    pub ovfs_work_dir: OvfsWorkDir,
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
        Zone::load_impl(mzr_dir, &zone_dir)
    }

    pub fn load_if_exists(mzr_dir: &MzrDir, zone_name: &ZoneName) -> Result<Option<Zone>, Error> {
        let zone_dir = ZoneDir::new(mzr_dir, &zone_name);
        if zone_dir.is_dir() {
            Ok(Some(Zone::load_impl(mzr_dir, &zone_dir)?))
        } else {
            Ok(None)
        }
    }

    /*
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
            Zone::load_impl(mzr_dir, &zone_dir)
        } else {
            let snap_name = get_snap_name()?;
            Zone::create_impl(mzr_dir, &zone_dir, zone_name, &snap_name)
        }
    }
    */

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
        let zone_parent = zone_dir.parent().ok_or(format_err!(
            "Unexpected error: zone directory must have a parent."
        ))?;
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
                let changes_dir = ChangesDir::new(&zone_dir);
                let ovfs_work_dir = OvfsWorkDir::new(&zone_dir);
                create_dir_all(changes_dir.clone()).context(format_err!(
                    "Unexpected error while creating zone changes directory {}",
                    changes_dir
                ))?;
                create_dir_all(ovfs_work_dir.clone()).context(format_err!(
                    "Unexpected error while creating zone ovfs work directory {}",
                    ovfs_work_dir
                ))?;
                let info = ZoneInfo {
                    snapshot: snap_name.clone(),
                    creation_time: Utc::now(),
                };
                json::write(&ZoneInfoFile::new(&zone_dir), &info)?;
                Ok(Zone {
                    zone_dir: zone_dir.clone(),
                    snap_dir,
                    changes_dir,
                    ovfs_work_dir,
                    info,
                })
            }
        }
    }

    pub fn load_impl(mzr_dir: &MzrDir, zone_dir: &ZoneDir) -> Result<Zone, Error> {
        let info: ZoneInfo = json::read(&ZoneInfoFile::new(&zone_dir))?.contents;
        let snap_dir = SnapDir::new(mzr_dir, &info.snapshot);
        let changes_dir = ChangesDir::new(zone_dir);
        let ovfs_work_dir = OvfsWorkDir::new(zone_dir);
        Ok(Zone {
            zone_dir: zone_dir.clone(),
            snap_dir,
            changes_dir,
            ovfs_work_dir,
            info,
        })
    }

    pub fn mount(&self, work_dir: &UserWorkDir) -> Result<(), Error> {
        Overlay::writable(
            iter::once(self.snap_dir.as_ref()),
            &self.changes_dir,
            &self.ovfs_work_dir,
            &work_dir,
        ).mount()
            // TODO(cleanup): Should make it so that '?' can be used,
            // by making libmount Error implement Sync.
            .map_err(|e| format_err!("{}", e))
    }
}
