use std::collections::BTreeMap;
use std::time::Duration;

use serde::Deserialize;

use crate::data::get;
use crate::error::Error;
use crate::model::{
    Celsius, Centimeters, Daylight, Forecast, HourlyForecast, KilometersPerHour, Meters,
    Millimeters, Percent, Point, Sky, UnixTime,
};

const URL: &str = "https://api.open-meteo.com/v1/forecast";
const HOURLY: &str = "temperature_2m,precipitation,precipitation_probability,snowfall,snow_depth,\
wind_speed_10m,wind_gusts_10m,visibility,sunshine_duration,freezing_level_height,snowfall_height,\
weather_code";

// Pressure levels, from about 550 m to about 5600 m.
const LEVELS: [&str; 8] = ["950", "925", "900", "850", "800", "700", "600", "500"];

// Open-Meteo sends one list per value, all in the same order as `time`.
#[derive(Deserialize)]
struct Answer {
    utc_offset_seconds: i32,
    hourly: Hourly,
    daily: Daily,
}

#[derive(Deserialize)]
struct Hourly {
    time: Vec<i64>,
    temperature_2m: Vec<Option<f64>>,
    precipitation: Vec<Option<f64>>,
    precipitation_probability: Vec<Option<u8>>,
    snowfall: Vec<Option<f64>>,
    snow_depth: Vec<Option<f64>>,
    wind_speed_10m: Vec<Option<f64>>,
    wind_gusts_10m: Vec<Option<f64>>,
    visibility: Vec<Option<f64>>,
    sunshine_duration: Vec<Option<f64>>,
    freezing_level_height: Vec<Option<f64>>,
    snowfall_height: Vec<Option<f64>>,
    weather_code: Vec<Option<u8>>,
    // Wind speed and height of every pressure level.
    #[serde(flatten)]
    levels: BTreeMap<String, Vec<Option<f64>>>,
}

#[derive(Deserialize)]
struct Daily {
    sunrise: Vec<i64>,
    sunset: Vec<i64>,
}

pub fn request(point: Point) -> Result<Forecast, Error> {
    let hourly = hourly_variables();
    let json = get(
        URL,
        &[
            ("latitude", &point.latitude.to_string()),
            ("longitude", &point.longitude.to_string()),
            ("elevation", &point.elevation.0.to_string()),
            ("hourly", &hourly),
            ("daily", "sunrise,sunset"),
            ("timeformat", "unixtime"),
            ("timezone", "auto"),
            ("forecast_days", "7"),
        ],
    )?;
    parse(point, &json)
}

fn hourly_variables() -> String {
    let mut variables = String::from(HOURLY);
    for level in LEVELS {
        variables.push(',');
        variables.push_str(&wind_key(level));
        variables.push(',');
        variables.push_str(&height_key(level));
    }
    variables
}

fn wind_key(level: &str) -> String {
    format!("wind_speed_{level}hPa")
}

fn height_key(level: &str) -> String {
    format!("geopotential_height_{level}hPa")
}

fn parse(point: Point, json: &str) -> Result<Forecast, Error> {
    let answer: Answer = serde_json::from_str(json).map_err(Error::UnexpectedData)?;
    let hours = (0..answer.hourly.time.len())
        .map(|index| hour(&answer.hourly, index, point.elevation))
        .collect::<Result<Vec<_>, _>>()?;
    let days = answer
        .daily
        .sunrise
        .into_iter()
        .zip(answer.daily.sunset)
        .map(|(sunrise, sunset)| Daylight {
            sunrise: UnixTime(sunrise),
            sunset: UnixTime(sunset),
        })
        .collect();
    Ok(Forecast {
        point,
        utc_offset_seconds: answer.utc_offset_seconds,
        hours,
        days,
    })
}

fn hour(hourly: &Hourly, index: usize, elevation: Meters) -> Result<HourlyForecast, Error> {
    let number = |list: &[Option<f64>], name| required(list, index, name);
    let sunshine = number(&hourly.sunshine_duration, "sunshine_duration")?;
    Ok(HourlyForecast {
        time: hourly
            .time
            .get(index)
            .copied()
            .map(UnixTime)
            .ok_or(Error::MissingData("time"))?,
        sky: sky(required(&hourly.weather_code, index, "weather_code")?)?,
        temperature: Celsius(number(&hourly.temperature_2m, "temperature_2m")?),
        precipitation: Millimeters(number(&hourly.precipitation, "precipitation")?),
        fresh_snow: Centimeters(number(&hourly.snowfall, "snowfall")?),
        snow_depth: Meters(number(&hourly.snow_depth, "snow_depth")?),
        wind: KilometersPerHour(number(&hourly.wind_speed_10m, "wind_speed_10m")?),
        gusts: KilometersPerHour(number(&hourly.wind_gusts_10m, "wind_gusts_10m")?),
        high_wind: high_wind(&hourly.levels, index, elevation),
        visibility: Meters(number(&hourly.visibility, "visibility")?),
        sunshine: Duration::try_from_secs_f64(sunshine)
            .map_err(|_| Error::MissingData("sunshine_duration"))?,
        freezing_level: Meters(number(
            &hourly.freezing_level_height,
            "freezing_level_height",
        )?),
        snow_line: optional(&hourly.snowfall_height, index).map(Meters),
        precipitation_probability: optional(&hourly.precipitation_probability, index).map(Percent),
    })
}

fn high_wind(
    levels: &BTreeMap<String, Vec<Option<f64>>>,
    index: usize,
    elevation: Meters,
) -> Option<KilometersPerHour> {
    let value = |key: String| levels.get(&key).and_then(|list| optional(list, index));
    let known: Vec<(f64, f64)> = LEVELS
        .iter()
        .filter_map(|level| Some((value(height_key(level))?, value(wind_key(level))?)))
        .collect();
    interpolate(&known, elevation.0).map(KilometersPerHour)
}

// Straight line between the levels just below and just above; `None` outside of them.
fn interpolate(levels: &[(f64, f64)], height: f64) -> Option<f64> {
    levels
        .iter()
        .zip(levels.iter().skip(1))
        .find(|((low, _), (high, _))| low < high && (*low..=*high).contains(&height))
        .map(|((low, below), (high, above))| {
            below + (above - below) * (height - low) / (high - low)
        })
}

fn required<T: Copy>(list: &[Option<T>], index: usize, name: &'static str) -> Result<T, Error> {
    optional(list, index).ok_or(Error::MissingData(name))
}

fn optional<T: Copy>(list: &[Option<T>], index: usize) -> Option<T> {
    list.get(index).copied().flatten()
}

// Codes of the World Meteorological Organization, as used by Open-Meteo.
fn sky(code: u8) -> Result<Sky, Error> {
    match code {
        0 | 1 => Ok(Sky::Clear),
        2 | 3 => Ok(Sky::Cloudy),
        45 | 48 => Ok(Sky::Fog),
        51 | 53 | 55 | 56 | 57 => Ok(Sky::Drizzle),
        61 | 63 | 65 | 66 | 67 => Ok(Sky::Rain),
        71 | 73 | 75 | 77 => Ok(Sky::Snow),
        80..=82 => Ok(Sky::RainShowers),
        85 | 86 => Ok(Sky::SnowShowers),
        95 | 96 | 99 => Ok(Sky::Thunderstorm),
        unknown => Err(Error::UnknownWeatherCode(unknown)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POINT: Point = Point {
        latitude: 47.4031,
        longitude: 9.8132,
        elevation: Meters(1448.0),
    };

    const BOEDELE: &str = include_str!("../../tests/data/forecast_boedele.json");
    const ANETO: &str = include_str!("../../tests/data/forecast_aneto.json");

    #[test]
    fn reads_seven_days() {
        let forecast = parse(POINT, BOEDELE).unwrap();

        assert_eq!(forecast.point, POINT);
        assert_eq!(forecast.utc_offset_seconds, 3600);
        assert_eq!(forecast.hours.len(), 168);
        assert_eq!(forecast.days.len(), 7);
        assert_eq!(forecast.days[0].sunrise, UnixTime(1795329305));
    }

    #[test]
    fn reads_first_hour() {
        let forecast = parse(POINT, BOEDELE).unwrap();
        let first = &forecast.hours[0];

        assert_eq!(first.time, UnixTime(1795302000));
        assert_eq!(first.sky, Sky::Clear);
        assert_eq!(first.temperature, Celsius(0.3));
        assert_eq!(first.wind, KilometersPerHour(6.5));
        assert_eq!(first.gusts, KilometersPerHour(23.4));
        assert_eq!(first.freezing_level, Meters(1930.0));
        assert_eq!(first.snow_line, Some(Meters(1060.0)));
        assert_eq!(first.precipitation_probability, Some(Percent(0)));
    }

    #[test]
    fn snow_line_may_be_missing() {
        let boedele = parse(POINT, BOEDELE).unwrap();
        let aneto = parse(POINT, ANETO).unwrap();

        assert_eq!(boedele.hours[167].snow_line, None);
        assert!(aneto.hours.iter().all(|hour| hour.snow_line.is_none()));
        assert_eq!(aneto.hours[0].sky, Sky::Snow);
    }

    #[test]
    fn missing_temperature_is_an_error() {
        let json = ANETO.replacen("\"temperature_2m\":[-4.9", "\"temperature_2m\":[null", 1);

        assert!(matches!(
            parse(POINT, &json),
            Err(Error::MissingData("temperature_2m"))
        ));
    }

    #[test]
    fn high_wind_is_stronger_than_ground_wind() {
        let aneto = Point {
            elevation: Meters(3404.0),
            ..POINT
        };
        let first = &parse(aneto, ANETO).unwrap().hours[0];

        assert_eq!(first.wind, KilometersPerHour(35.6));
        let high_wind = first.high_wind.unwrap().0;
        assert!((high_wind - 76.264).abs() < 0.001);
    }

    #[test]
    fn interpolates_between_levels() {
        let levels = [(1000.0, 10.0), (1500.0, 20.0), (3000.0, 50.0)];

        assert_eq!(interpolate(&levels, 1000.0), Some(10.0));
        assert_eq!(interpolate(&levels, 1250.0), Some(15.0));
        assert_eq!(interpolate(&levels, 3000.0), Some(50.0));
        assert_eq!(interpolate(&levels, 900.0), None);
        assert_eq!(interpolate(&levels, 3001.0), None);
        assert_eq!(interpolate(&[], 1000.0), None);
    }

    #[test]
    fn rejects_unknown_weather_code() {
        assert!(matches!(sky(4), Err(Error::UnknownWeatherCode(4))));
        assert_eq!(sky(99).unwrap(), Sky::Thunderstorm);
    }
}
