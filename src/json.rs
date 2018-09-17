use chrono::{DateTime, Utc};
use failure::Error;
use semver::Version;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_derive::{Deserialize, Serialize};
use serde_json;
use std::fs::File;
use std::path::PathBuf;

const VERSION_STRING: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonFile<T> {
    pub contents: T,
    pub writer: WriterInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WriterInfo {
    pub program: String,
    pub mzr_version: Version,
    pub update_time: DateTime<Utc>,
}

pub fn write<T: Serialize>(path: &PathBuf, value: &T) -> Result<(), Error> {
    serde_json::to_writer_pretty(
        File::create(path)?,
        &JsonFile {
            contents: value,
            writer: WriterInfo {
                program: String::from("mzr"),
                mzr_version: Version::parse(VERSION_STRING)?,
                update_time: Utc::now(),
            },
        },
    )?;
    Ok(())
}

pub fn read<T>(path: &PathBuf) -> Result<JsonFile<T>, Error>
where
    T: DeserializeOwned,
{
    Ok(serde_json::from_reader(File::open(path)?)?)
}
