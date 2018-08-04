use failure::Error;
use libmount::Overlay;
use std::fs::create_dir_all;
use std::iter;

use paths::*;
use top_dirs::TopDirs;

#[derive(Debug)]
pub struct Zone {
    pub zone_dir: ZoneDir,
    pub snap_dir: SnapDir,
    pub changes_dir: ChangesDir,
    pub ovfs_work_dir: OvfsWorkDir,
    pub user_work_dir: UserWorkDir,
}

impl Zone {
    // TODO(snapshots): Should load some of this stuff from files in the zone
    // dir instead of trusting the correspondance.
    pub fn load(top_dirs: &TopDirs, zone_name: &ZoneName) -> Result<Zone, Error> {
        let zone_dir = ZoneDir::new(&top_dirs.mizer_dir, &zone_name);
        let snap_dir = SnapDir::new(&top_dirs.user_root_dir);
        let changes_dir = ChangesDir::new(&zone_dir);
        let ovfs_work_dir = OvfsWorkDir::new(&zone_dir);
        // Create dirs if necessary.
        create_dir_all(zone_dir.clone())?;
        create_dir_all(snap_dir.clone())?;
        create_dir_all(changes_dir.clone())?;
        create_dir_all(ovfs_work_dir.clone())?;
        Ok(Zone {
            zone_dir,
            snap_dir,
            changes_dir,
            ovfs_work_dir,
            user_work_dir: top_dirs.user_work_dir.clone(),
        })
    }

    pub fn mount(&self) -> Result<(), Error> {
        Overlay::writable(
            iter::once(self.snap_dir.as_ref()),
            &self.changes_dir,
            &self.ovfs_work_dir,
            &self.user_work_dir,
        ).mount()
            // TODO(cleanup): Should make it so that '?' can be used, by making
            // libmount Error implement Sync.
            .map_err(|e| format_err!("{}", e))
    }

    /*
    fn load_impl(zone_dir: ZoneDir) {
    }

    fn create_impl(zone_dir: ZoneDir, snap_dir: SnapDir) {
    }
    */
}

// TODO(nice-errors): These should include more info

/*
#[derive(Debug, Fail)]
#[fail(display = "Zone already exists")]
struct ZoneAlreadyExists;

#[derive(Debug, Fail)]
#[fail(display = "Zone not found")]
struct ZoneNotFound;
*/
