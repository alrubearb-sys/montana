mod areas;
mod data;
mod error;
mod model;
mod storage;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::data::elevation::Source;
use crate::data::forecast;
use crate::data::search::Place;
use crate::error::Error;
use crate::model::{Forecast, HourlyForecast, Point, UnixTime};

/// Should I go to the mountains?
#[derive(Parser)]
#[command(
    version,
    args_conflicts_with_subcommands = true,
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Area to check, for example "bodele"
    area: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    /// Search for a place in Europe
    Search { text: String },
    /// Save a place as an area
    Add {
        /// Place to search for
        text: String,
        /// Number of the search result to save
        #[arg(long, default_value_t = 1)]
        result: usize,
        /// Name for the area, by default the name of the place
        #[arg(long)]
        name: Option<String>,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Error> {
    match (cli.command, cli.area) {
        (Some(Command::Search { text }), _) => search(&text),
        (Some(Command::Add { text, result, name }), _) => add(&text, result, name),
        (None, Some(area)) => show(&area),
        (None, None) => Ok(()),
    }
}

fn search(text: &str) -> Result<(), Error> {
    let places = areas::search(text)?;
    if places.is_empty() {
        return Err(Error::NoResults(text.to_string()));
    }
    for (number, place) in (1..).zip(&places) {
        print_place(number, place);
    }
    println!("\nSearch data © OpenStreetMap contributors (ODbL)");
    Ok(())
}

fn add(text: &str, chosen: usize, name: Option<String>) -> Result<(), Error> {
    let (area, source) = areas::add(text, chosen, name)?;
    println!("Saved: {}", area.name);
    print_point("Summit", area.summit);
    println!("Elevation from {}", credit(source));
    Ok(())
}

fn show(name: &str) -> Result<(), Error> {
    let area = areas::find(name)?;
    println!("{}", area.name);
    print_forecast("Summit", &forecast::request(area.summit)?);
    if let Some(base) = area.base {
        print_forecast("Base", &forecast::request(base)?);
    }
    println!("\nWeather data by Open-Meteo (CC BY 4.0)");
    Ok(())
}

fn print_place(number: usize, place: &Place) {
    let elevation = match place.elevation {
        Some(elevation) => format!("{} m", elevation.0),
        None => String::from("no elevation"),
    };
    println!(
        "{number}. {} ({}, {elevation}): {}",
        place.name, place.kind, place.description
    );
}

fn print_point(label: &str, point: Point) {
    println!(
        "{label}: {} m ({}, {})",
        point.elevation.0, point.latitude, point.longitude
    );
}

fn print_forecast(label: &str, forecast: &Forecast) {
    println!();
    print_point(label, forecast.point);
    for day in &forecast.days {
        println!(
            "{} sun {}-{}",
            weekday(day.sunrise, forecast.utc_offset_seconds),
            clock(day.sunrise, forecast.utc_offset_seconds),
            clock(day.sunset, forecast.utc_offset_seconds)
        );
    }
    for hour in &forecast.hours {
        print_hour(hour, forecast.utc_offset_seconds);
    }
}

fn print_hour(hour: &HourlyForecast, offset: i32) {
    let snow_line = hour
        .snow_line
        .map_or(String::from("-"), |line| format!("{} m", line.0));
    let probability = hour
        .precipitation_probability
        .map_or(String::from("-"), |percent| format!("{} %", percent.0));
    let high_wind = hour
        .high_wind
        .map_or(String::from("-"), |wind| format!("{:.0} km/h", wind.0));
    println!(
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
    );
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

fn credit(source: Source) -> &'static str {
    match source {
        Source::OpenStreetMap => "© OpenStreetMap contributors (ODbL)",
        Source::Swisstopo => "© swisstopo",
        Source::OpenMeteo => "Open-Meteo (CC BY 4.0)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_local_day_and_time() {
        let midnight = UnixTime(1795302000);

        assert_eq!(weekday(midnight, 3600), "Sun");
        assert_eq!(clock(midnight, 3600), "00:00");
        assert_eq!(weekday(midnight, 0), "Sat");
        assert_eq!(clock(midnight, 0), "23:00");
    }
}
