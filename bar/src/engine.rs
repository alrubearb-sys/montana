use std::env;
use std::ffi::OsString;
use std::fmt;
use std::process::Command;

use serde::Deserialize;
use serde::de::DeserializeOwned;

/// The panel of an area, worked out by the engine: everything comes ready to show,
/// so the panels of the Mac and of Omarchy only draw it.
#[derive(Debug, Clone, Deserialize)]
pub struct Panel {
    pub utc_offset_seconds: i64,
    pub daylight: String,
    pub snow_line: String,
    pub freezing_level: String,
    pub snow_depth: String,
    pub spots: Vec<Spot>,
    pub days: Vec<Day>,
    pub marks: Vec<Mark>,
    pub temperature: Chart,
    pub wind: Chart,
    pub precipitation: Chart,
    pub readings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Spot {
    pub label: String,
    pub height: String,
    pub sky: String,
    pub temperature: String,
    pub wind: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Day {
    pub name: String,
    pub sky: String,
    pub temperature: String,
    pub wind: String,
    pub sun: String,
    pub snow: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Mark {
    pub position: f32,
    pub text: String,
    pub midnight: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Chart {
    pub legend: Vec<Legend>,
    pub line: String,
    pub snow: String,
    pub rain: String,
    pub ice: String,
    pub ticks: Vec<Tick>,
    /// Where 0 °C is, from the top; only in the temperature chart.
    pub zero: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Legend {
    pub text: String,
    pub tone: Tone,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Line,
    Snow,
    Ice,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Tick {
    pub position: f32,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SavedArea {
    pub name: String,
    pub summit: Point,
    pub base: Option<Point>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Place {
    pub name: String,
    pub kind: String,
    pub description: String,
    pub elevation: Option<f64>,
}

#[derive(Deserialize)]
struct SearchResults {
    places: Vec<Place>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Point {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug)]
pub enum Error {
    BadData(serde_json::Error),
    Failed(String),
    NotFound(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadData(source) => write!(f, "cannot read the answer of peakcast: {source}"),
            Error::Failed(message) => write!(f, "{message}"),
            Error::NotFound(source) => write!(f, "cannot run peakcast: {source}"),
        }
    }
}

pub fn areas() -> Result<Vec<SavedArea>, Error> {
    run(&["areas", "--json"])
}

pub fn panel(area: &str) -> Result<Panel, Error> {
    run(&["panel", "--", area])
}

pub fn search(text: &str) -> Result<Vec<Place>, Error> {
    run::<SearchResults>(&["search", "--json", "--", text]).map(|results| results.places)
}

// `number` counts from 1, as in `peakcast search`. The search is cached, so this
// does not ask Nominatim again.
pub fn add(text: &str, number: usize) -> Result<(), Error> {
    execute(&["add", "--result", &number.to_string(), "--", text]).map(|_| ())
}

pub fn open_map(center: &Point) -> Result<Option<String>, Error> {
    map(&[degrees(center)])
}

pub fn edit_map(area: &SavedArea) -> Result<Option<String>, Error> {
    let mut arguments = vec![
        String::from("--edit"),
        area.name.clone(),
        degrees(&area.summit),
    ];
    arguments.extend(area.base.as_ref().map(degrees));
    map(&arguments)
}

fn degrees(point: &Point) -> String {
    format!("{},{}", point.latitude, point.longitude)
}

// Waits until the map is closed, and gives the name of the area saved in it, if any.
fn map(arguments: &[String]) -> Result<Option<String>, Error> {
    let output = Command::new(program("peakcast-map"))
        .args(arguments)
        .output()
        .map_err(Error::NotFound)?;
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(Some(name).filter(|name| output.status.success() && !name.is_empty()))
}

// The newest release, if this one is older. Without network there is none.
pub fn outdated() -> Option<String> {
    let output = execute(&["outdated"]).ok()?;
    let version = String::from_utf8_lossy(&output).trim().to_string();
    Some(version).filter(|version| !version.is_empty())
}

// In Terminal, as Omarchy updates itself in one, so that what brew does is seen.
// Peakcast opens again at the end, updated or not.
pub fn update() -> Result<(), Error> {
    let status = Command::new("osascript")
        .args([
            "-e",
            r#"tell application "Terminal" to do script "brew upgrade --cask peakcast; open -a Peakcast""#,
            "-e",
            r#"tell application "Terminal" to activate"#,
        ])
        .status()
        .map_err(Error::NotFound)?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::Failed(String::from(
            "cannot open Terminal to update",
        )))
    }
}

// `position` counts from 1, as in `peakcast order`.
pub fn order(area: &str, position: usize) -> Result<(), Error> {
    execute(&["order", "--", area, &position.to_string()]).map(|_| ())
}

pub fn remove(area: &str) -> Result<(), Error> {
    execute(&["remove", "--", area]).map(|_| ())
}

fn run<T: DeserializeOwned>(arguments: &[&str]) -> Result<T, Error> {
    serde_json::from_slice(&execute(arguments)?).map_err(Error::BadData)
}

// A separate process, so that the memory of the network connection is given back.
fn execute(arguments: &[&str]) -> Result<Vec<u8>, Error> {
    let output = Command::new(program("peakcast"))
        .args(arguments)
        .output()
        .map_err(Error::NotFound)?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        let message = String::from_utf8_lossy(&output.stderr);
        Err(Error::Failed(message.trim().to_string()))
    }
}

// Next to the panel inside Peakcast.app, or else on the PATH.
fn program(name: &str) -> OsString {
    env::current_exe()
        .ok()
        .map(|path| path.with_file_name(name))
        .filter(|path| path.is_file())
        .map_or_else(|| OsString::from(name), OsString::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_saved_areas() {
        let json = r#"[
            {"name": "Bödele ski resort",
             "summit": {"latitude": 47.4031, "longitude": 9.8132, "elevation": 1448.0},
             "base": {"latitude": 47.4178, "longitude": 9.8182, "elevation": 1088.0}},
            {"name": "St. Gallen (Lachen)", "kind": "suburb",
             "summit": {"latitude": 47.4193, "longitude": 9.3519, "elevation": 663.4},
             "base": null}
        ]"#;

        let areas: Vec<SavedArea> = serde_json::from_str(json).unwrap();

        assert_eq!(areas[0].name, "Bödele ski resort");
        assert!(areas[0].base.is_some());
        assert_eq!(areas[1].summit.latitude, 47.4193);
        assert!(areas[1].base.is_none());
    }

    #[test]
    fn reads_the_panel_of_the_engine() {
        let json = r#"{
            "name": "Bödele Ski", "utc_offset_seconds": 3600, "daylight": "07:36 - 16:37",
            "snow_line": "1100 m", "freezing_level": "1100 m", "snow_depth": "4 cm",
            "spots": [{"label": "summit", "height": "1453 m", "sky": "fog",
                       "temperature": "-3 °C", "wind": "12 km/h"}],
            "days": [], "marks": [{"position": 0.25, "text": "18h", "midnight": false}],
            "chart_width": 370.0, "chart_height": 68.0,
            "temperature": {"legend": [{"text": "temperature °C", "tone": "line"}],
                            "line": "M 0 34 L 370 34 ", "snow": "", "rain": "", "ice": "",
                            "ticks": [{"position": 0.5, "text": "0"}], "zero": 0.5},
            "wind": {"legend": [], "line": "", "snow": "", "rain": "", "ice": "",
                     "ticks": [], "zero": null},
            "precipitation": {"legend": [{"text": "snow", "tone": "snow"}], "line": "",
                              "snow": "", "rain": "", "ice": "", "ticks": [], "zero": null},
            "readings": ["Mon 18:00  -3 °C  wind 12 km/h"],
            "credits": "Weather data by Open-Meteo (CC BY 4.0)"
        }"#;

        let panel: Panel = serde_json::from_str(json).unwrap();

        assert_eq!(panel.utc_offset_seconds, 3600);
        assert_eq!(panel.spots[0].temperature, "-3 °C");
        assert_eq!(panel.temperature.zero, Some(0.5));
        assert_eq!(panel.wind.zero, None);
        assert!(matches!(panel.precipitation.legend[0].tone, Tone::Snow));
    }
}
