use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::data::search::Place;
use crate::error::Error;
use crate::model::Area;

#[derive(Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct Config {
    pub search_url: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            search_url: String::from("https://nominatim.openstreetmap.org/search"),
        }
    }
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SearchCache {
    pub last_request_ms: u64,
    pub results: BTreeMap<String, Vec<Place>>,
}

pub fn load_config() -> Result<Config, Error> {
    load(&config_dir()?.join("config.json"))
}

pub fn load_areas() -> Result<Vec<Area>, Error> {
    load(&config_dir()?.join("areas.json"))
}

pub fn save_areas(areas: &[Area]) -> Result<(), Error> {
    save(&config_dir()?.join("areas.json"), areas)
}

pub fn load_cache() -> Result<SearchCache, Error> {
    load(&cache_dir()?.join("search.json"))
}

pub fn save_cache(cache: &SearchCache) -> Result<(), Error> {
    save(&cache_dir()?.join("search.json"), cache)
}

fn config_dir() -> Result<PathBuf, Error> {
    directory("XDG_CONFIG_HOME", ".config")
}

fn cache_dir() -> Result<PathBuf, Error> {
    directory("XDG_CACHE_HOME", ".cache")
}

fn directory(variable: &str, fallback: &str) -> Result<PathBuf, Error> {
    let base = match env::var_os(variable).filter(|value| !value.is_empty()) {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(env::var_os("HOME").ok_or(Error::MissingHome)?).join(fallback),
    };
    Ok(base.join("montana"))
}

fn load<T: DeserializeOwned + Default>(path: &Path) -> Result<T, Error> {
    match fs::read_to_string(path) {
        Ok(json) => {
            serde_json::from_str(&json).map_err(|source| Error::BadFile(path.to_path_buf(), source))
        }
        Err(source) if source.kind() == ErrorKind::NotFound => Ok(T::default()),
        Err(source) => Err(Error::File(path.to_path_buf(), source)),
    }
}

fn save<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), Error> {
    let failed = |source| Error::File(path.to_path_buf(), source);
    let json = serde_json::to_string_pretty(value).map_err(Error::UnexpectedData)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(failed)?;
    }
    // Write aside and rename, so that a crash never leaves half a file.
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, json).map_err(failed)?;
    fs::rename(&temporary, path).map_err(failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Meters, Point};

    #[test]
    fn missing_file_gives_default() {
        let config: Config = load(Path::new("/nonexistent/montana/config.json")).unwrap();

        assert_eq!(config, Config::default());
    }

    #[test]
    fn saved_areas_are_read_back() {
        let directory = env::temp_dir().join(format!("montana-test-{}", std::process::id()));
        let path = directory.join("areas.json");
        let areas = vec![Area {
            name: String::from("Aneto"),
            summit: Point {
                latitude: 42.6311,
                longitude: 0.6567,
                elevation: Meters(3404.0),
            },
            base: None,
        }];

        save(&path, areas.as_slice()).unwrap();
        let read: Vec<Area> = load(&path).unwrap();
        fs::remove_dir_all(&directory).unwrap();

        assert_eq!(read, areas);
    }
}
