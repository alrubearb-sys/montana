use serde::Deserialize;

use crate::data::get;
use crate::data::search::Place;
use crate::error::Error;
use crate::model::Meters;

const SWISSTOPO: &str = "https://api3.geo.admin.ch/rest/services/height";
// West, east, south and north of Switzerland. swisstopo also answers a little
// beyond the border (Bödele, in Austria: 1456 m), so the box is enough.
const SWITZERLAND: (f64, f64, f64, f64) = (5.9, 10.6, 45.8, 47.9);
const OPEN_METEO: &str = "https://api.open-meteo.com/v1/elevation";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    OpenStreetMap,
    Swisstopo,
    OpenMeteo,
}

#[derive(Deserialize)]
struct SwisstopoHeight {
    height: String,
}

#[derive(Deserialize)]
struct OpenMeteoElevation {
    elevation: Vec<f64>,
}

pub fn find(place: &Place) -> Result<(Meters, Source), Error> {
    if let Some(elevation) = place.elevation {
        return Ok((elevation, Source::OpenStreetMap));
    }
    if place.country_code == "ch" {
        return swisstopo(place.latitude, place.longitude);
    }
    open_meteo(place.latitude, place.longitude)
}

// For a point picked on the map, whose country is not known.
pub fn at(latitude: f64, longitude: f64) -> Result<(Meters, Source), Error> {
    if in_switzerland(latitude, longitude) {
        swisstopo(latitude, longitude).or_else(|_| open_meteo(latitude, longitude))
    } else {
        open_meteo(latitude, longitude)
    }
}

fn in_switzerland(latitude: f64, longitude: f64) -> bool {
    let (west, east, south, north) = SWITZERLAND;
    (west..=east).contains(&longitude) && (south..=north).contains(&latitude)
}

fn swisstopo(latitude: f64, longitude: f64) -> Result<(Meters, Source), Error> {
    let (easting, northing) = to_lv95(latitude, longitude);
    let json = get(
        SWISSTOPO,
        &[
            ("easting", &format!("{easting:.1}")),
            ("northing", &format!("{northing:.1}")),
            ("sr", "2056"),
        ],
    )?;
    Ok((parse_swisstopo(&json)?, Source::Swisstopo))
}

fn open_meteo(latitude: f64, longitude: f64) -> Result<(Meters, Source), Error> {
    let json = get(
        OPEN_METEO,
        &[
            ("latitude", &latitude.to_string()),
            ("longitude", &longitude.to_string()),
        ],
    )?;
    Ok((parse_open_meteo(&json)?, Source::OpenMeteo))
}

fn parse_swisstopo(json: &str) -> Result<Meters, Error> {
    let answer: SwisstopoHeight = serde_json::from_str(json).map_err(Error::UnexpectedData)?;
    answer
        .height
        .parse()
        .map(Meters)
        .map_err(|_| Error::NoElevation)
}

fn parse_open_meteo(json: &str) -> Result<Meters, Error> {
    let answer: OpenMeteoElevation = serde_json::from_str(json).map_err(Error::UnexpectedData)?;
    answer
        .elevation
        .first()
        .copied()
        .map(Meters)
        .ok_or(Error::NoElevation)
}

// Approximate swisstopo formula from GPS degrees to Swiss coordinates, accurate to about 1 m.
fn to_lv95(latitude: f64, longitude: f64) -> (f64, f64) {
    let lat = (latitude * 3600.0 - 169_028.66) / 10_000.0;
    let lon = (longitude * 3600.0 - 26_782.5) / 10_000.0;
    let easting = 2_600_072.37 + 211_455.93 * lon
        - 10_938.51 * lon * lat
        - 0.36 * lon * lat.powi(2)
        - 44.54 * lon.powi(3);
    let northing = 1_200_147.07 + 308_807.95 * lat + 3_745.25 * lon.powi(2) + 76.63 * lat.powi(2)
        - 194.56 * lon.powi(2) * lat
        + 119.79 * lat.powi(3);
    (easting, northing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switzerland_box_takes_border_points() {
        assert!(in_switzerland(47.2494, 9.3432));
        assert!(in_switzerland(47.4031, 9.8132));
        assert!(!in_switzerland(42.6311, 0.6567));
    }

    #[test]
    fn reads_swisstopo_height() {
        let json = include_str!("../../tests/data/swisstopo_height.json");

        assert_eq!(parse_swisstopo(json).unwrap(), Meters(1144.0));
    }

    #[test]
    fn reads_open_meteo_elevation() {
        let json = include_str!("../../tests/data/open_meteo_elevation.json");

        assert_eq!(parse_open_meteo(json).unwrap(), Meters(1088.0));
    }

    #[test]
    fn missing_elevation_is_an_error() {
        assert!(matches!(
            parse_open_meteo("{\"elevation\": []}"),
            Err(Error::NoElevation)
        ));
    }

    #[test]
    fn converts_degrees_to_swiss_coordinates() {
        let latitude = 46.0 + 2.0 / 60.0 + 38.87 / 3600.0;
        let longitude = 8.0 + 43.0 / 60.0 + 49.79 / 3600.0;

        let (easting, northing) = to_lv95(latitude, longitude);

        assert!((easting - 2_700_000.0).abs() < 1.0);
        assert!((northing - 1_100_000.0).abs() < 1.0);
    }
}
