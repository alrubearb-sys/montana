use std::time::Duration;

use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Meters(pub f64);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub latitude: f64,
    pub longitude: f64,
    pub elevation: Meters,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Area {
    pub name: String,
    // What the place is, as Nominatim says ("city", "suburb", "peak"). Areas saved
    // before it was kept have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub summit: Point,
    pub base: Option<Point>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Celsius(pub f64);

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Millimeters(pub f64);

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Centimeters(pub f64);

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct KilometersPerHour(pub f64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Percent(pub u8);

/// Seconds since 1 January 1970 UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct UnixTime(pub i64);

// From better to worse: the panel picks the worse sky between two equally usual ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HourlyForecast {
    pub time: UnixTime,
    pub sky: Sky,
    #[serde(rename = "temperature_c")]
    pub temperature: Celsius,
    #[serde(rename = "precipitation_mm")]
    pub precipitation: Millimeters,
    #[serde(rename = "fresh_snow_cm")]
    pub fresh_snow: Centimeters,
    #[serde(rename = "snow_depth_m")]
    pub snow_depth: Meters,
    #[serde(rename = "wind_kmh")]
    pub wind: KilometersPerHour,
    #[serde(rename = "gusts_kmh")]
    pub gusts: KilometersPerHour,
    /// Wind in the free air at the elevation of the point, not slowed down by the ground.
    #[serde(rename = "high_wind_kmh")]
    pub high_wind: Option<KilometersPerHour>,
    #[serde(rename = "visibility_m")]
    pub visibility: Meters,
    #[serde(rename = "sunshine_minutes", serialize_with = "as_minutes")]
    pub sunshine: Duration,
    #[serde(rename = "freezing_level_m")]
    pub freezing_level: Meters,
    // Open-Meteo does not provide these two for every hour.
    #[serde(rename = "snow_line_m")]
    pub snow_line: Option<Meters>,
    #[serde(rename = "precipitation_probability_percent")]
    pub precipitation_probability: Option<Percent>,
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

impl HourlyForecast {
    /// The wind at the height of the point when the forecast has it.
    pub fn best_wind(&self) -> f64 {
        self.high_wind.unwrap_or(self.wind).0
    }

    /// The amounts decide. When they do not point to snow, the sky does.
    pub fn fall(&self) -> Fall {
        if self.precipitation.0 <= 0.0 {
            return Fall::Nothing;
        }
        if self.fresh_snow.0 * WATER_MM_PER_SNOW_CM * 2.0 >= self.precipitation.0 {
            return Fall::Snow;
        }
        match self.sky {
            Sky::FreezingDrizzle | Sky::FreezingRain => Fall::Freezing,
            Sky::Snow | Sky::SnowShowers if self.temperature.0 <= 0.0 => Fall::Snow,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Daylight {
    pub sunrise: UnixTime,
    pub sunset: UnixTime,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Forecast {
    pub point: Point,
    pub utc_offset_seconds: i32,
    pub hours: Vec<HourlyForecast>,
    pub days: Vec<Daylight>,
}

fn as_minutes<S: Serializer>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_u64(duration.as_secs() / 60)
}
