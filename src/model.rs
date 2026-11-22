use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Meters(pub f64);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub latitude: f64,
    pub longitude: f64,
    pub elevation: Meters,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Area {
    pub name: String,
    pub base: Point,
    pub summit: Point,
}

#[expect(dead_code, reason = "used by the ranking in phase 3")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Ski,
    Mountain,
}

#[expect(dead_code, reason = "filled in when the forecast is fetched")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Celsius(pub f64);

#[expect(dead_code, reason = "filled in when the forecast is fetched")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Millimeters(pub f64);

#[expect(dead_code, reason = "filled in when the forecast is fetched")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Centimeters(pub f64);

#[expect(dead_code, reason = "filled in when the forecast is fetched")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KilometersPerHour(pub f64);

#[expect(dead_code, reason = "filled in when the forecast is fetched")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Percent(pub u8);

/// Seconds since 1 January 1970 UTC.
#[expect(dead_code, reason = "filled in when the forecast is fetched")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnixTime(pub i64);

#[expect(dead_code, reason = "filled in when the forecast is fetched")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sky {
    Clear,
    Cloudy,
    Fog,
    Drizzle,
    Rain,
    Snow,
    RainShowers,
    SnowShowers,
    Thunderstorm,
}

#[expect(dead_code, reason = "filled in when the forecast is fetched")]
#[derive(Debug, Clone, PartialEq)]
pub struct HourlyForecast {
    pub time: UnixTime,
    pub sky: Sky,
    pub temperature: Celsius,
    pub precipitation: Millimeters,
    pub fresh_snow: Centimeters,
    pub snow_depth: Meters,
    pub wind: KilometersPerHour,
    pub gusts: KilometersPerHour,
    pub visibility: Meters,
    pub sunshine: Duration,
    pub freezing_level: Meters,
    // Open-Meteo does not provide these two for every hour.
    pub snow_line: Option<Meters>,
    pub precipitation_probability: Option<Percent>,
}

#[expect(dead_code, reason = "filled in when the forecast is fetched")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Daylight {
    pub sunrise: UnixTime,
    pub sunset: UnixTime,
}

#[expect(dead_code, reason = "filled in when the forecast is fetched")]
#[derive(Debug, Clone, PartialEq)]
pub struct Forecast {
    pub point: Point,
    pub hours: Vec<HourlyForecast>,
    pub days: Vec<Daylight>,
}
