use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::data::elevation::{self, Source};
use crate::data::search::{self, Place};
use crate::error::Error;
use crate::model::{Area, Meters, Point};
use crate::storage;

// Nominatim allows one request per second at most.
const REQUEST_INTERVAL_MS: u64 = 1100;

pub fn find(name: &str) -> Result<Area, Error> {
    let wanted = name.to_lowercase();
    let saved = storage::load_areas()?
        .into_iter()
        .find(|area| area.name.to_lowercase() == wanted);
    match (saved, wanted.as_str()) {
        (Some(area), _) => Ok(area),
        (None, "bodele" | "boedele" | "bödele") => Ok(boedele()),
        (None, _) => Err(Error::UnknownArea(name.to_string())),
    }
}

pub fn search(text: &str) -> Result<Vec<Place>, Error> {
    let key = text.trim().to_lowercase();
    let mut cache = storage::load_cache()?;
    if let Some(places) = cache.results.get(&key) {
        return Ok(places.clone());
    }

    let config = storage::load_config()?;
    thread::sleep(Duration::from_millis(wait_ms(
        cache.last_request_ms,
        now_ms(),
    )));
    cache.last_request_ms = now_ms();
    storage::save_cache(&cache)?;

    let places = search::request(&config.search_url, text.trim())?;
    cache.results.insert(key, places.clone());
    storage::save_cache(&cache)?;
    Ok(places)
}

pub fn add(text: &str, chosen: usize, name: Option<String>) -> Result<(Area, Source), Error> {
    let mut places = search(text)?;
    if places.is_empty() {
        return Err(Error::NoResults(text.to_string()));
    }
    let available = places.len();
    let index = chosen
        .checked_sub(1)
        .filter(|index| *index < available)
        .ok_or(Error::ResultOutOfRange { chosen, available })?;
    let place = places.swap_remove(index);

    let name = name.unwrap_or_else(|| place.name.clone());
    let mut areas = storage::load_areas()?;
    if areas
        .iter()
        .any(|area| area.name.to_lowercase() == name.to_lowercase())
    {
        return Err(Error::AlreadySaved(name));
    }

    let (elevation, source) = elevation::find(&place)?;
    let area = Area {
        name,
        summit: Point {
            latitude: place.latitude,
            longitude: place.longitude,
            elevation,
        },
        base: None,
    };
    areas.push(area.clone());
    storage::save_areas(&areas)?;
    Ok((area, source))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            elapsed.as_secs() * 1000 + u64::from(elapsed.subsec_millis())
        })
}

fn wait_ms(last_request_ms: u64, now_ms: u64) -> u64 {
    match now_ms.checked_sub(last_request_ms) {
        Some(elapsed) => REQUEST_INTERVAL_MS.saturating_sub(elapsed),
        // The clock went back: wait the whole interval.
        None => REQUEST_INTERVAL_MS,
    }
}

fn boedele() -> Area {
    Area {
        name: String::from("Bödele ski resort"),
        summit: Point {
            latitude: 47.4031,
            longitude: 9.8132,
            elevation: Meters(1448.0),
        },
        base: Some(Point {
            latitude: 47.4178,
            longitude: 9.8182,
            elevation: Meters(1088.0),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_between_requests() {
        assert_eq!(wait_ms(0, 1_000_000), 0);
        assert_eq!(wait_ms(10_000, 10_000), 1100);
        assert_eq!(wait_ms(10_000, 10_600), 500);
        assert_eq!(wait_ms(10_000, 9_000), 1100);
    }
}
