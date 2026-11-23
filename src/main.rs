mod areas;
mod data;
mod error;
mod model;
mod output;
mod storage;

use std::io::{self, ErrorKind, Write};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::data::forecast;
use crate::error::Error;
use crate::model::Area;
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
    /// Search for a place in Europe
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

fn list(out: &mut impl Write, json: bool) -> Result<(), Error> {
    let names: Vec<String> = areas::all()?.into_iter().map(|area| area.name).collect();
    if json {
        writeln!(out, "{}", output::to_json(&names)?)?;
    } else {
        for name in names {
            writeln!(out, "{name}")?;
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

fn report(area: Area) -> Result<Report, Error> {
    Ok(Report {
        name: area.name,
        summit: forecast::request(area.summit)?,
        base: area.base.map(forecast::request).transpose()?,
        credits: output::WEATHER_CREDIT,
    })
}
