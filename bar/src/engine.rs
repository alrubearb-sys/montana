use std::env;
use std::ffi::OsString;
use std::fmt;
use std::process::Command;

use serde::Deserialize;
use serde::de::DeserializeOwned;

#[derive(Debug, Clone, Deserialize)]
pub struct Report {
    #[serde(default)]
    pub kind: Option<String>,
    pub summit: Forecast,
    pub base: Option<Forecast>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Forecast {
    pub point: Point,
    pub utc_offset_seconds: i64,
    pub hours: Vec<Hour>,
    pub days: Vec<Daylight>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Place {
    pub name: String,
    pub kind: String,
    pub description: String,
    pub elevation: Option<f64>,
}

#[derive(Deserialize)]
struct SearchResults {
    places: Vec<Place>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Point {
    pub elevation: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Hour {
    pub time: i64,
    pub sky: Sky,
    pub temperature_c: f64,
    pub precipitation_mm: f64,
    pub fresh_snow_cm: f64,
    pub snow_depth_m: f64,
    pub wind_kmh: f64,
    pub high_wind_kmh: Option<f64>,
    pub sunshine_minutes: f64,
    pub freezing_level_m: f64,
    pub snow_line_m: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sky {
    Clear,
    Cloudy,
    Fog,
    Drizzle,
    FreezingDrizzle,
    Rain,
    FreezingRain,
    Snow,
    RainShowers,
    SnowShowers,
    Thunderstorm,
}

impl Sky {
    pub fn label(self) -> &'static str {
        match self {
            Sky::Clear => "clear",
            Sky::Cloudy => "cloudy",
            Sky::Fog => "fog",
            Sky::Drizzle => "drizzle",
            Sky::FreezingDrizzle => "icy drizzle",
            Sky::Rain => "rain",
            Sky::FreezingRain => "icy rain",
            Sky::Snow => "snow",
            Sky::RainShowers => "rain showers",
            Sky::SnowShowers => "snow showers",
            Sky::Thunderstorm => "thunderstorm",
        }
    }
}

/// What falls during most of an hour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fall {
    Nothing,
    Rain,
    Freezing,
    Snow,
}

// Open-Meteo counts 7 cm of fresh snow as 10 mm of water.
const WATER_MM_PER_SNOW_CM: f64 = 10.0 / 7.0;

impl Hour {
    pub fn best_wind(&self) -> f64 {
        self.high_wind_kmh.unwrap_or(self.wind_kmh)
    }

    /// The amounts decide. When they do not point to snow, the sky does.
    pub fn fall(&self) -> Fall {
        if self.precipitation_mm <= 0.0 {
            return Fall::Nothing;
        }
        if self.fresh_snow_cm * WATER_MM_PER_SNOW_CM * 2.0 >= self.precipitation_mm {
            return Fall::Snow;
        }
        match self.sky {
            Sky::FreezingDrizzle | Sky::FreezingRain => Fall::Freezing,
            Sky::Snow | Sky::SnowShowers if self.temperature_c <= 0.0 => Fall::Snow,
            Sky::Clear
            | Sky::Cloudy
            | Sky::Fog
            | Sky::Drizzle
            | Sky::Rain
            | Sky::Snow
            | Sky::RainShowers
            | Sky::SnowShowers
            | Sky::Thunderstorm => Fall::Rain,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Daylight {
    pub sunrise: i64,
    pub sunset: i64,
}

#[derive(Debug)]
pub enum Error {
    BadData(serde_json::Error),
    Failed(String),
    NotFound(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadData(source) => write!(f, "cannot read the answer of montana: {source}"),
            Error::Failed(message) => write!(f, "{message}"),
            Error::NotFound(source) => write!(f, "cannot run montana: {source}"),
        }
    }
}

pub fn areas() -> Result<Vec<String>, Error> {
    run(&["areas", "--json"])
}

pub fn report(area: &str) -> Result<Report, Error> {
    run(&["--json", "--", area])
}

pub fn search(text: &str) -> Result<Vec<Place>, Error> {
    run::<SearchResults>(&["search", "--json", "--", text]).map(|results| results.places)
}

// `number` counts from 1, as in `montana search`. The search is cached, so this
// does not ask Nominatim again.
pub fn add(text: &str, number: usize) -> Result<(), Error> {
    execute(&["add", "--result", &number.to_string(), "--", text]).map(|_| ())
}

pub fn remove(area: &str) -> Result<(), Error> {
    execute(&["remove", "--", area]).map(|_| ())
}

fn run<T: DeserializeOwned>(arguments: &[&str]) -> Result<T, Error> {
    serde_json::from_slice(&execute(arguments)?).map_err(Error::BadData)
}

// A separate process, so that the memory of the network connection is given back.
fn execute(arguments: &[&str]) -> Result<Vec<u8>, Error> {
    let output = Command::new(program())
        .args(arguments)
        .output()
        .map_err(Error::NotFound)?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        let message = String::from_utf8_lossy(&output.stderr);
        Err(Error::Failed(message.trim().to_string()))
    }
}

fn program() -> OsString {
    env::current_exe()
        .ok()
        .map(|path| path.with_file_name("montana"))
        .filter(|path| path.is_file())
        .map_or_else(|| OsString::from("montana"), OsString::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Hours of the Bödele summit on 22 and 23 November 2026.
    fn fall(sky: Sky, temperature_c: f64, precipitation_mm: f64, fresh_snow_cm: f64) -> Fall {
        Hour {
            time: 0,
            sky,
            temperature_c,
            precipitation_mm,
            fresh_snow_cm,
            snow_depth_m: 0.0,
            wind_kmh: 0.0,
            high_wind_kmh: None,
            sunshine_minutes: 0.0,
            freezing_level_m: 0.0,
            snow_line_m: None,
        }
        .fall()
    }

    #[test]
    fn says_what_falls_most() {
        assert_eq!(fall(Sky::Cloudy, 2.0, 0.0, 0.0), Fall::Nothing);
        assert_eq!(fall(Sky::Rain, 0.3, 0.7, 0.0), Fall::Rain);
        assert_eq!(fall(Sky::Snow, -1.2, 1.5, 0.84), Fall::Snow);
        assert_eq!(fall(Sky::Rain, 0.8, 1.5, 0.2), Fall::Rain);
    }

    #[test]
    fn freezing_drizzle_is_not_rain_nor_snow() {
        assert_eq!(fall(Sky::FreezingDrizzle, -2.9, 0.4, 0.0), Fall::Freezing);
        assert_eq!(fall(Sky::FreezingDrizzle, -2.9, 0.6, 0.42), Fall::Snow);
    }

    #[test]
    fn snow_showers_below_zero_are_snow() {
        assert_eq!(fall(Sky::SnowShowers, -2.6, 0.4, 0.0), Fall::Snow);
        assert_eq!(fall(Sky::SnowShowers, 1.0, 0.4, 0.0), Fall::Rain);
    }
}
