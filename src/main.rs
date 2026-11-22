mod areas;
mod data;
mod error;
mod model;
mod output;
mod storage;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::data::forecast;
use crate::error::Error;
use crate::model::Area;
use crate::output::Report;

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
        (None, Some(area)) if cli.json => show_json(&area),
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
        output::print_place(number, place);
    }
    println!("\nSearch data © OpenStreetMap contributors (ODbL)");
    Ok(())
}

fn add(text: &str, chosen: usize, name: Option<String>) -> Result<(), Error> {
    let (area, source) = areas::add(text, chosen, name)?;
    println!("Saved: {}", area.name);
    output::print_point("Summit", area.summit);
    println!("Elevation from {}", output::credit(source));
    Ok(())
}

fn show(name: &str) -> Result<(), Error> {
    let report = report(areas::find(name)?)?;
    output::print_report(&report);
    Ok(())
}

fn show_json(name: &str) -> Result<(), Error> {
    println!("{}", output::to_json(&report(areas::find(name)?)?)?);
    Ok(())
}

fn report(area: Area) -> Result<Report, Error> {
    Ok(Report {
        name: area.name,
        summit: forecast::request(area.summit)?,
        base: area.base.map(forecast::request).transpose()?,
        credits: output::WEATHER_CREDIT,
    })
}
