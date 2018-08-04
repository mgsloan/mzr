use failure::{Error, ResultExt};
use paths::{MizerDir, UserRootDir, UserWorkDir};
use std::env;
use std::fs::create_dir_all;
use std::path::PathBuf;
use utils::confirm;

#[derive(Debug, Clone)]
pub struct TopDirs {
    pub mizer_dir: MizerDir,
    pub user_work_dir: UserWorkDir,
    pub user_root_dir: UserRootDir,
}

impl TopDirs {
    /*
    pub fn find() -> Result<TopDirs, Error> {
        TopDirs::find_impl(&current_dir()?)
    } */

    fn find_impl(start_dir: &PathBuf) -> Result<TopDirs, Error> {
        let mut dir = start_dir.clone();
        loop {
            let candidate = TopDirs::from_user_root(UserRootDir::new(&dir));
            if candidate.mizer_dir.is_dir() {
                //TODO(cleanup): can this clone be avoided? (same on other
                // create_dir_all usages)
                create_dir_all(candidate.user_work_dir.clone())?;
                return Ok(candidate);
            }
            dir.pop();
            if dir.file_name().is_none() {
                return Err(MizerDirNotFound.into());
            }
        }
    }

    pub fn find_or_prompt_create(action: &str) -> Result<TopDirs, Error> {
        let start_dir = current_dir()?;
        match TopDirs::find_impl(&start_dir) {
            Ok(top_dirs) => Ok(top_dirs),
            Err(err) => {
                match err.downcast() {
                    Ok(MizerDirNotFound) => {
                        let dirs = TopDirs::from_user_root(UserRootDir::new(&start_dir));
                        println!("Couldn't find a mizer directory sibling to any parent directory");
                        if confirm(&format!("Initialize a new mizer dir at {}", dirs.mizer_dir))? {
                            create_dir_all(dirs.mizer_dir.clone())?;
                            create_dir_all(dirs.user_work_dir.clone())?;
                            println!("Mizer directory initialized.");
                            //TODO(cleanup): can this clone be avoided?
                            Ok(dirs.clone())
                        } else {
                            Err(format_err!("Can't {} without a mizer directory", action))
                        }
                    }
                    Err(other_err) => Err(other_err),
                }
            }
        }
    }

    fn from_user_root(user_root_dir: UserRootDir) -> TopDirs {
        TopDirs {
            mizer_dir: MizerDir::new(&user_root_dir),
            user_work_dir: UserWorkDir::new(&user_root_dir),
            user_root_dir,
        }
    }
}

#[derive(Fail, Debug)]
#[fail(display = "Did not find mizer directory for any parent directories.")]
pub struct MizerDirNotFound;

/// Like `env::current_dir`, but gives a decent error.
fn current_dir() -> Result<PathBuf, Error> {
    Ok(env::current_dir().context("Error getting current directory - does it still exist?")?)
}
