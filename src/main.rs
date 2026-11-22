mod areas;
mod model;

use std::process::ExitCode;

use clap::Parser;

use crate::model::Point;

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
    print_point("Summit", area.summit);
    if let Some(base) = area.base {
        print_point("Base", base);
    }
    ExitCode::SUCCESS
}

fn print_point(label: &str, point: Point) {
    println!(
        "{label}: {} m ({}, {})",
        point.elevation.0, point.latitude, point.longitude
    );
}
