mod areas;
mod data;
mod error;
mod model;
mod storage;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::data::elevation::Source;
use crate::data::search::Place;
use crate::error::Error;
use crate::model::Point;

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
    print_point("Summit", area.summit);
    if let Some(base) = area.base {
        print_point("Base", base);
    }
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

fn credit(source: Source) -> &'static str {
    match source {
        Source::OpenStreetMap => "© OpenStreetMap contributors (ODbL)",
        Source::Swisstopo => "© swisstopo",
        Source::OpenMeteo => "Open-Meteo (CC BY 4.0)",
    }
}
