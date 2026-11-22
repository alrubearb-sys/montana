mod areas;
mod model;

use std::process::ExitCode;

use clap::Parser;

/// Should I go to the mountains?
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Area to check, for example "bodele"
    area: String,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let Some(area) = areas::find(&cli.area) else {
        eprintln!("Unknown area: {}", cli.area);
        return ExitCode::FAILURE;
    };

    println!("{}", area.name);
    for (label, point) in [("Base", area.base), ("Summit", area.summit)] {
        println!(
            "{label}: {} m ({}, {})",
            point.elevation.0, point.latitude, point.longitude
        );
    }
    ExitCode::SUCCESS
}
