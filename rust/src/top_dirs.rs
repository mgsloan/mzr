use failure::{Error, ResultExt};
use paths::{MzrDir, UserWorkDir};
use std::env;
use std::fs::create_dir_all;
use std::path::PathBuf;
use utils::{confirm, Confirmed};

#[derive(Debug, Clone)]
pub struct TopDirs {
    pub mzr_dir: MzrDir,
    pub user_work_dir: UserWorkDir,
}

impl TopDirs {
    /*
    pub fn find() -> Result<TopDirs, Error> {
        TopDirs::find_impl(&current_dir()?)
    } */

    fn find_impl(start_dir: &PathBuf) -> Result<TopDirs, Error> {
        let mut dir = start_dir.clone();
        loop {
            let candidate = TopDirs::from_user_work(UserWorkDir::new(&dir));
            if candidate.mzr_dir.is_dir() {
                return Ok(candidate);
            }
            dir.pop();
            if dir.file_name().is_none() {
                return Err(MzrDirNotFound.into());
            }
        }
    }

    pub fn find_or_prompt_create(action: &str) -> Result<TopDirs, Error> {
        let start_dir = env::var_os("MZR_DIR")
            .map(|v| v.into())
            .unwrap_or(current_dir()?);
        match TopDirs::find_impl(&start_dir) {
            Ok(top_dirs) => Ok(top_dirs),
            Err(err) => {
                match err.downcast() {
                    Ok(MzrDirNotFound) => {
                        let dirs = match find_git_repo(&start_dir) {
                            None => {
                                println!("Couldn't find a mzr directory sibling to any parent directory.");
                                TopDirs::from_user_work(UserWorkDir::new(&start_dir))
                            }
                            Some(git_dir) => {
                                println!("Couldn't find a mzr directory sibling to any parent directory, \
                                          but did find a git repository at {}", git_dir);
                                TopDirs::from_user_work(git_dir)
                            }
                        };
                        match confirm(&format!("Initialize a new mzr dir at {}", dirs.mzr_dir))? {
                            Confirmed::Yes => {
                                //TODO(cleanup): can this clone be avoided? (same on other
                                // create_dir_all usages)
                                create_dir_all(dirs.mzr_dir.clone())?;
                                create_dir_all(dirs.user_work_dir.clone())?;
                                println!("Mzr directory initialized.");
                                //TODO(cleanup): can this clone be avoided?
                                Ok(dirs.clone())
                            }
                            Confirmed::No => {
                                Err(format_err!("Can't {} without a mzr directory", action))
                            }
                        }
                    }
                    Err(other_err) => Err(other_err),
                }
            }
        }
    }

    fn from_user_work(user_work_dir: UserWorkDir) -> TopDirs {
        TopDirs {
            mzr_dir: MzrDir::new(&user_work_dir),
            user_work_dir,
        }
    }
}

#[derive(Fail, Debug)]
#[fail(display = "Did not find mzr directory for any parent directories.")]
pub struct MzrDirNotFound;

/// Like `env::current_dir`, but gives a decent error.
fn current_dir() -> Result<PathBuf, Error> {
    Ok(env::current_dir().context("Error getting current directory - does it still exist?")?)
}

fn find_git_repo(start_dir: &PathBuf) -> Option<UserWorkDir> {
    let mut cur = start_dir.clone();
    loop {
        cur.push(".git");
        // Note that this intentionally includes files, since ".git" files
        // are used for git work-trees.
        if cur.exists() {
            cur.pop();
            return Some(UserWorkDir::new(&cur));
        }
        cur.pop();
        cur.pop();
        if cur.file_name().is_none() {
            return None;
        }
    }
}
