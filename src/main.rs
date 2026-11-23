mod areas;
mod clock;
mod data;
mod error;
mod model;
mod output;
mod panel;
mod storage;

use std::io::{self, ErrorKind, Write};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand};

use crate::data::forecast;
use crate::error::Error;
use crate::model::{Area, UnixTime};
use crate::output::{Report, SearchResults};

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

    /// Print the forecast as JSON
    #[arg(long, requires = "area")]
    json: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Search for a place
    Search {
        text: String,
        /// Print the places as JSON
        #[arg(long)]
        json: bool,
    },
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
    /// Remove a saved area
    Remove { name: String },
    /// Save a new area from coordinates
    Create {
        name: String,
        /// Summit, as latitude,longitude
        #[arg(long, value_parser = coordinates, allow_hyphen_values = true)]
        summit: (f64, f64),
        /// Base, as latitude,longitude
        #[arg(long, value_parser = coordinates, allow_hyphen_values = true)]
        base: Option<(f64, f64)>,
        /// What the place is, for example "peak"
        #[arg(long)]
        kind: Option<String>,
    },
    /// Move the summit or the base of a saved area
    Move {
        name: String,
        /// New summit, as latitude,longitude
        #[arg(long, value_parser = coordinates, allow_hyphen_values = true)]
        summit: (f64, f64),
        /// New base, as latitude,longitude; without it the base stays
        #[arg(long, value_parser = coordinates, allow_hyphen_values = true)]
        base: Option<(f64, f64)>,
        /// New name for the area
        #[arg(long)]
        rename: Option<String>,
    },
    /// Print what the panels show for an area, as JSON
    Panel { area: String },
    /// List the areas
    Areas {
        /// Print the names as JSON
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse(), &mut io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        // Whoever was reading the output has stopped, as `head` does.
        Err(Error::Output(error)) if error.kind() == ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli, out: &mut impl Write) -> Result<(), Error> {
    match (cli.command, cli.area) {
        (Some(Command::Search { text, json }), _) => search(out, &text, json),
        (Some(Command::Add { text, result, name }), _) => add(out, &text, result, name),
        (Some(Command::Remove { name }), _) => remove(out, &name),
        (
            Some(Command::Move {
                name,
                summit,
                base,
                rename,
            }),
            _,
        ) => move_area(out, &name, rename.as_deref(), summit, base),
        (
            Some(Command::Create {
                name,
                summit,
                base,
                kind,
            }),
            _,
        ) => create(out, &name, kind, summit, base),
        (Some(Command::Panel { area }), _) => show_panel(out, &area),
        (Some(Command::Areas { json }), _) => list(out, json),
        (None, Some(area)) if cli.json => show_json(out, &area),
        (None, Some(area)) => show(out, &area),
        (None, None) => Ok(()),
    }
}

fn search(out: &mut impl Write, text: &str, json: bool) -> Result<(), Error> {
    let places = areas::search(text)?;
    if json {
        // No results is an answer for the panel, not an error.
        let results = SearchResults {
            places,
            credits: output::SEARCH_CREDIT,
        };
        writeln!(out, "{}", output::to_json(&results)?)?;
        return Ok(());
    }
    if places.is_empty() {
        return Err(Error::NoResults(text.to_string()));
    }
    for (number, place) in (1..).zip(&places) {
        output::print_place(out, number, place)?;
    }
    writeln!(out, "\n{}", output::SEARCH_CREDIT)?;
    Ok(())
}

fn add(out: &mut impl Write, text: &str, chosen: usize, name: Option<String>) -> Result<(), Error> {
    let (area, source) = areas::add(text, chosen, name)?;
    writeln!(out, "Saved: {}", area.name)?;
    output::print_point(out, "Summit", area.summit)?;
    writeln!(out, "Elevation from {}", output::credit(source))?;
    Ok(())
}

fn remove(out: &mut impl Write, name: &str) -> Result<(), Error> {
    let area = areas::remove(name)?;
    writeln!(out, "Removed: {}", area.name)?;
    Ok(())
}

fn create(
    out: &mut impl Write,
    name: &str,
    kind: Option<String>,
    summit: (f64, f64),
    base: Option<(f64, f64)>,
) -> Result<(), Error> {
    let area = areas::create(name, kind, summit, base)?;
    writeln!(out, "Saved: {}", area.name)?;
    output::print_point(out, "Summit", area.summit)?;
    if let Some(base) = area.base {
        output::print_point(out, "Base", base)?;
    }
    Ok(())
}

fn move_area(
    out: &mut impl Write,
    name: &str,
    new_name: Option<&str>,
    summit: (f64, f64),
    base: Option<(f64, f64)>,
) -> Result<(), Error> {
    let area = areas::move_to(name, new_name, summit, base)?;
    writeln!(out, "Moved: {}", area.name)?;
    output::print_point(out, "Summit", area.summit)?;
    if let Some(base) = area.base {
        output::print_point(out, "Base", base)?;
    }
    Ok(())
}

// "47.2494,9.3432" as clap reads it: a message instead of our Error, because clap
// shows it next to the option.
fn coordinates(text: &str) -> Result<(f64, f64), String> {
    let invalid = || format!("expected latitude,longitude in degrees, got {text}");
    let (latitude, longitude) = text.split_once(',').ok_or_else(invalid)?;
    let latitude: f64 = latitude.trim().parse().map_err(|_| invalid())?;
    let longitude: f64 = longitude.trim().parse().map_err(|_| invalid())?;
    if (-90.0..=90.0).contains(&latitude) && (-180.0..=180.0).contains(&longitude) {
        Ok((latitude, longitude))
    } else {
        Err(invalid())
    }
}

// The JSON has the whole areas, so that the panel knows their points before any
// forecast is asked for.
fn list(out: &mut impl Write, json: bool) -> Result<(), Error> {
    let areas = areas::all()?;
    if json {
        writeln!(out, "{}", output::to_json(&areas)?)?;
    } else {
        for area in areas {
            writeln!(out, "{}", area.name)?;
        }
    }
    Ok(())
}

fn show(out: &mut impl Write, name: &str) -> Result<(), Error> {
    let report = report(areas::find(name)?)?;
    output::print_report(out, &report)?;
    Ok(())
}

fn show_json(out: &mut impl Write, name: &str) -> Result<(), Error> {
    writeln!(out, "{}", output::to_json(&report(areas::find(name)?)?)?)?;
    Ok(())
}

fn show_panel(out: &mut impl Write, name: &str) -> Result<(), Error> {
    let report = report(areas::find(name)?)?;
    writeln!(out, "{}", output::to_json(&panel::build(&report, now()))?)?;
    Ok(())
}

// A clock before 1970 would be broken; it then counts as 1970.
fn now() -> UnixTime {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    UnixTime(i64::try_from(seconds).unwrap_or(i64::MAX))
}

fn report(area: Area) -> Result<Report, Error> {
    Ok(Report {
        name: area.name,
        kind: area.kind,
        summit: forecast::request(area.summit)?,
        base: area.base.map(forecast::request).transpose()?,
        credits: output::WEATHER_CREDIT,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_coordinates() {
        assert_eq!(coordinates("47.2494,9.3432"), Ok((47.2494, 9.3432)));
        assert_eq!(coordinates("40.4, -3.7"), Ok((40.4, -3.7)));
        assert!(coordinates("47.2494").is_err());
        assert!(coordinates("north,east").is_err());
        assert!(coordinates("95,9").is_err());
    }
}
