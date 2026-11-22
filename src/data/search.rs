use serde::{Deserialize, Serialize};

use crate::data::get;
use crate::error::Error;
use crate::model::Meters;

// Left, top, right and bottom of Europe, in degrees.
const EUROPE: &str = "-11,71.5,40,34";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Place {
    pub name: String,
    pub kind: String,
    pub description: String,
    pub country_code: String,
    pub latitude: f64,
    pub longitude: f64,
    pub elevation: Option<Meters>,
}

#[derive(Deserialize)]
struct NominatimPlace {
    lat: String,
    lon: String,
    name: String,
    #[serde(rename = "type")]
    kind: String,
    display_name: String,
    #[serde(default)]
    address: Address,
    extratags: Option<ExtraTags>,
}

#[derive(Default, Deserialize)]
struct Address {
    #[serde(default)]
    country_code: String,
}

#[derive(Deserialize)]
struct ExtraTags {
    ele: Option<String>,
}

pub fn request(url: &str, text: &str) -> Result<Vec<Place>, Error> {
    let query = [
        ("q", text),
        ("format", "jsonv2"),
        ("limit", "5"),
        ("extratags", "1"),
        ("addressdetails", "1"),
        ("accept-language", "en"),
        ("viewbox", EUROPE),
        ("bounded", "1"),
    ];
    parse(&get(url, &query)?)
}

fn parse(json: &str) -> Result<Vec<Place>, Error> {
    let places: Vec<NominatimPlace> = serde_json::from_str(json).map_err(Error::UnexpectedData)?;
    places.into_iter().map(to_place).collect()
}

fn to_place(place: NominatimPlace) -> Result<Place, Error> {
    Ok(Place {
        latitude: parse_degrees(&place.lat)?,
        longitude: parse_degrees(&place.lon)?,
        elevation: place
            .extratags
            .and_then(|tags| tags.ele)
            .and_then(|text| parse_elevation(&text)),
        name: name_or_start(place.name, &place.display_name),
        kind: place.kind,
        description: place.display_name,
        country_code: place.address.country_code,
    })
}

// Addresses and other places without a name of their own.
fn name_or_start(name: String, description: &str) -> String {
    if name.is_empty() {
        description
            .split(',')
            .next()
            .unwrap_or_default()
            .to_string()
    } else {
        name
    }
}

fn parse_degrees(text: &str) -> Result<f64, Error> {
    text.parse()
        .map_err(|_| Error::BadCoordinates(text.to_string()))
}

// OpenStreetMap is edited by hand: some elevations carry a unit, others are not numbers.
fn parse_elevation(text: &str) -> Option<Meters> {
    let number: f64 = text.trim().trim_end_matches('m').trim().parse().ok()?;
    number.is_finite().then_some(Meters(number))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_peak_with_elevation() {
        let places = parse(include_str!("../../tests/data/nominatim_aneto.json")).unwrap();

        assert_eq!(places.len(), 3);
        assert_eq!(places[0].name, "Aneto");
        assert_eq!(places[0].kind, "peak");
        assert_eq!(places[0].country_code, "es");
        assert_eq!(places[0].elevation, Some(Meters(3404.0)));
        assert_eq!(places[0].latitude, 42.6311462);
    }

    #[test]
    fn reads_place_without_elevation() {
        let places = parse(include_str!("../../tests/data/nominatim_santis.json")).unwrap();

        assert_eq!(places[0].elevation, Some(Meters(2502.0)));
        assert_eq!(places[2].elevation, None);
    }

    #[test]
    fn reads_empty_answer() {
        let places = parse(include_str!("../../tests/data/nominatim_empty.json")).unwrap();

        assert!(places.is_empty());
    }

    #[test]
    fn rejects_other_data() {
        assert!(matches!(
            parse("{\"error\": \"blocked\"}"),
            Err(Error::UnexpectedData(_))
        ));
    }

    #[test]
    fn unnamed_place_takes_start_of_description() {
        let name = name_or_start(String::new(), "12, Dorfstrasse, Zermatt");

        assert_eq!(name, "12");
        assert_eq!(
            name_or_start(String::from("Aneto"), "Aneto, Spain"),
            "Aneto"
        );
    }

    #[test]
    fn reads_hand_written_elevations() {
        assert_eq!(parse_elevation("4807.3"), Some(Meters(4807.3)));
        assert_eq!(parse_elevation("1188 m"), Some(Meters(1188.0)));
        assert_eq!(parse_elevation("about 2000"), None);
        assert_eq!(parse_elevation("NaN"), None);
    }
}
