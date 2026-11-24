use std::env;
use std::ffi::OsString;
use std::fmt;
use std::io;
use std::process::Command;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Place {
    pub name: String,
    pub kind: String,
    pub latitude: f64,
    pub longitude: f64,
    // Older engines do not write it.
    #[serde(default)]
    pub detail: String,
}

#[derive(Deserialize)]
struct SearchResults {
    places: Vec<Place>,
}

#[derive(Debug)]
pub enum Error {
    BadData(serde_json::Error),
    Failed(String),
    NotFound(io::Error),
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

pub fn search(text: &str) -> Result<Vec<Place>, Error> {
    let output = execute(&["search", "--json", "--", text])?;
    let results: SearchResults = serde_json::from_slice(&output).map_err(Error::BadData)?;
    Ok(results.places)
}

pub fn create(
    name: &str,
    kind: Option<&str>,
    summit: (f64, f64),
    base: Option<(f64, f64)>,
) -> Result<(), Error> {
    let summit = degrees(summit);
    let base = base.map(degrees);
    let mut arguments = vec!["create", "--summit", &summit];
    if let Some(base) = &base {
        arguments.extend(["--base", base]);
    }
    if let Some(kind) = kind {
        arguments.extend(["--kind", kind]);
    }
    arguments.extend(["--", name]);
    execute(&arguments).map(|_| ())
}

pub fn move_area(
    area: &str,
    new_name: &str,
    summit: (f64, f64),
    base: Option<(f64, f64)>,
) -> Result<(), Error> {
    let summit = degrees(summit);
    let base = base.map(degrees);
    let mut arguments = vec!["move", "--summit", &summit, "--rename", new_name];
    if let Some(base) = &base {
        arguments.extend(["--base", base]);
    }
    arguments.extend(["--", area]);
    execute(&arguments).map(|_| ())
}

fn degrees((latitude, longitude): (f64, f64)) -> String {
    format!("{latitude:.6},{longitude:.6}")
}

// The engine does the network and the saving, as for the panel.
fn execute(arguments: &[&str]) -> Result<Vec<u8>, Error> {
    let output = Command::new(program())
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

fn program() -> OsString {
    env::current_exe()
        .ok()
        .map(|path| path.with_file_name("peakcast"))
        .filter(|path| path.is_file())
        .map_or_else(|| OsString::from("peakcast"), OsString::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_found_places_with_their_detail() {
        let json = r#"{"places": [{"name": "Säntis", "kind": "peak",
            "description": "Säntis, Schwende-Rüte, Switzerland", "country_code": "ch",
            "latitude": 47.2494, "longitude": 9.3432, "elevation": 2502.0,
            "detail": "peak · 2502 m · Schwende-Rüte, Switzerland"}],
            "credits": "Search data © OpenStreetMap contributors (ODbL)"}"#;

        let results: SearchResults = serde_json::from_str(json).unwrap();

        assert_eq!(results.places[0].name, "Säntis");
        assert_eq!(
            results.places[0].detail,
            "peak · 2502 m · Schwende-Rüte, Switzerland"
        );
    }
}
