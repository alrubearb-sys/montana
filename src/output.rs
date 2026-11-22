use std::io::{self, Write};

use serde::Serialize;

use crate::data::elevation::Source;
use crate::data::search::Place;
use crate::error::Error;
use crate::model::{Forecast, HourlyForecast, Point, UnixTime};

pub const WEATHER_CREDIT: &str = "Weather data by Open-Meteo (CC BY 4.0)";

#[derive(Serialize)]
pub struct Report {
    pub name: String,
    pub summit: Forecast,
    pub base: Option<Forecast>,
    pub credits: &'static str,
}

pub fn to_json(report: &Report) -> Result<String, Error> {
    serde_json::to_string(report).map_err(Error::UnexpectedData)
}

pub fn print_report(out: &mut impl Write, report: &Report) -> io::Result<()> {
    writeln!(out, "{}", report.name)?;
    print_forecast(out, "Summit", &report.summit)?;
    if let Some(base) = &report.base {
        print_forecast(out, "Base", base)?;
    }
    writeln!(out, "\n{}", report.credits)?;
    Ok(())
}

pub fn print_place(out: &mut impl Write, number: usize, place: &Place) -> io::Result<()> {
    let elevation = match place.elevation {
        Some(elevation) => format!("{} m", elevation.0),
        None => String::from("no elevation"),
    };
    writeln!(
        out,
        "{number}. {} ({}, {elevation}): {}",
        place.name, place.kind, place.description
    )?;
    Ok(())
}

pub fn print_point(out: &mut impl Write, label: &str, point: Point) -> io::Result<()> {
    writeln!(
        out,
        "{label}: {} m ({}, {})",
        point.elevation.0, point.latitude, point.longitude
    )?;
    Ok(())
}

fn print_forecast(out: &mut impl Write, label: &str, forecast: &Forecast) -> io::Result<()> {
    writeln!(out)?;
    print_point(out, label, forecast.point)?;
    for day in &forecast.days {
        writeln!(
            out,
            "{} sun {}-{}",
            weekday(day.sunrise, forecast.utc_offset_seconds),
            clock(day.sunrise, forecast.utc_offset_seconds),
            clock(day.sunset, forecast.utc_offset_seconds)
        )?;
    }
    for hour in &forecast.hours {
        print_hour(out, hour, forecast.utc_offset_seconds)?;
    }
    Ok(())
}

fn print_hour(out: &mut impl Write, hour: &HourlyForecast, offset: i32) -> io::Result<()> {
    let snow_line = hour
        .snow_line
        .map_or(String::from("-"), |line| format!("{} m", line.0));
    let probability = hour
        .precipitation_probability
        .map_or(String::from("-"), |percent| format!("{} %", percent.0));
    let high_wind = hour
        .high_wind
        .map_or(String::from("-"), |wind| format!("{:.0} km/h", wind.0));
    writeln!(
        out,
        "{} {} {:?}, {} °C, rain or snow {} mm ({probability}), fresh snow {} cm, snow depth {} m, \
         wind {} km/h, gusts {} km/h, high wind {high_wind}, visibility {} m, sunshine {} min, freezing level {} m, \
         snow line {snow_line}",
        weekday(hour.time, offset),
        clock(hour.time, offset),
        hour.sky,
        hour.temperature.0,
        hour.precipitation.0,
        hour.fresh_snow.0,
        hour.snow_depth.0,
        hour.wind.0,
        hour.gusts.0,
        hour.visibility.0,
        hour.sunshine.as_secs() / 60,
        hour.freezing_level.0,
    )?;
    Ok(())
}

fn weekday(time: UnixTime, offset: i32) -> &'static str {
    let days = (time.0 + i64::from(offset)).div_euclid(86_400);
    // 1 January 1970 was a Thursday.
    match days.rem_euclid(7) {
        0 => "Thu",
        1 => "Fri",
        2 => "Sat",
        3 => "Sun",
        4 => "Mon",
        5 => "Tue",
        _ => "Wed",
    }
}

fn clock(time: UnixTime, offset: i32) -> String {
    let seconds = (time.0 + i64::from(offset)).rem_euclid(86_400);
    format!("{:02}:{:02}", seconds / 3600, seconds % 3600 / 60)
}

pub fn credit(source: Source) -> &'static str {
    match source {
        Source::OpenStreetMap => "© OpenStreetMap contributors (ODbL)",
        Source::Swisstopo => "© swisstopo",
        Source::OpenMeteo => "Open-Meteo (CC BY 4.0)",
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::model::{
        Celsius, Centimeters, Daylight, KilometersPerHour, Meters, Millimeters, Percent, Sky,
    };

    #[test]
    fn shows_local_day_and_time() {
        let midnight = UnixTime(1795302000);

        assert_eq!(weekday(midnight, 3600), "Sun");
        assert_eq!(clock(midnight, 3600), "00:00");
        assert_eq!(weekday(midnight, 0), "Sat");
        assert_eq!(clock(midnight, 0), "23:00");
    }

    #[test]
    fn json_has_units_and_null_for_missing_values() {
        let point = Point {
            latitude: 42.6311,
            longitude: 0.6567,
            elevation: Meters(3404.0),
        };
        let hour = HourlyForecast {
            time: UnixTime(1795302000),
            sky: Sky::RainShowers,
            temperature: Celsius(-4.9),
            precipitation: Millimeters(1.7),
            fresh_snow: Centimeters(0.42),
            snow_depth: Meters(0.0),
            wind: KilometersPerHour(35.6),
            gusts: KilometersPerHour(71.6),
            high_wind: Some(KilometersPerHour(76.0)),
            visibility: Meters(3640.0),
            sunshine: Duration::from_secs(3420),
            freezing_level: Meters(2770.0),
            snow_line: None,
            precipitation_probability: Some(Percent(93)),
        };
        let report = Report {
            name: String::from("Aneto"),
            summit: Forecast {
                point,
                utc_offset_seconds: 3600,
                hours: vec![hour],
                days: vec![Daylight {
                    sunrise: UnixTime(1795330631),
                    sunset: UnixTime(1795364940),
                }],
            },
            base: None,
            credits: WEATHER_CREDIT,
        };

        let json: serde_json::Value = serde_json::from_str(&to_json(&report).unwrap()).unwrap();

        let expected = serde_json::json!({
            "name": "Aneto",
            "summit": {
                "point": { "latitude": 42.6311, "longitude": 0.6567, "elevation": 3404.0 },
                "utc_offset_seconds": 3600,
                "hours": [{
                    "time": 1795302000,
                    "sky": "rain_showers",
                    "temperature_c": -4.9,
                    "precipitation_mm": 1.7,
                    "fresh_snow_cm": 0.42,
                    "snow_depth_m": 0.0,
                    "wind_kmh": 35.6,
                    "gusts_kmh": 71.6,
                    "high_wind_kmh": 76.0,
                    "visibility_m": 3640.0,
                    "sunshine_minutes": 57,
                    "freezing_level_m": 2770.0,
                    "snow_line_m": null,
                    "precipitation_probability_percent": 93
                }],
                "days": [{ "sunrise": 1795330631, "sunset": 1795364940 }]
            },
            "base": null,
            "credits": "Weather data by Open-Meteo (CC BY 4.0)"
        });
        assert_eq!(json, expected);
    }
}
