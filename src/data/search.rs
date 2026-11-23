use serde::{Deserialize, Serialize};

use crate::data::get;
use crate::error::Error;
use crate::model::Meters;

// More than are shown, so that the reordering below has something to choose from.
const ASKED: &str = "10";
const SHOWN: usize = 5;
// A nudge for mountain places when two are about as well known: Nominatim's
// importance, mostly from Wikipedia, goes from 0 to 1 (Mount Everest 0.61, a street
// called Everest 0.05).
const MOUNTAIN_BONUS: f64 = 0.1;
// Nominatim ranks places by size: 4 a country, 8 a state or canton, 12 a district,
// 16 a city, 18 a peak. Up to districts, the point is only the middle of a large
// area: the canton of St. Gallen gives a mountain at 2010 m.
const SMALLEST_REGION_RANK: u8 = 12;

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
    // The OpenStreetMap value, such as "peak" or "winter_sports".
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    importance: f64,
    // What the place is ("city", "peak", "postcode"), clearer than `type`.
    #[serde(default)]
    addresstype: String,
    place_rank: u8,
    display_name: String,
    #[serde(default)]
    address: Address,
    extratags: Option<ExtraTags>,
}

#[derive(Default, Deserialize)]
struct Address {
    #[serde(default)]
    country_code: String,
    village: Option<String>,
    town: Option<String>,
    city: Option<String>,
}

#[derive(Deserialize)]
struct ExtraTags {
    ele: Option<String>,
}

pub fn request(url: &str, text: &str) -> Result<Vec<Place>, Error> {
    let query = [
        ("q", text),
        ("format", "jsonv2"),
        ("limit", ASKED),
        ("extratags", "1"),
        ("addressdetails", "1"),
        ("accept-language", "en"),
    ];
    parse(&get(url, &query)?)
}

fn parse(json: &str) -> Result<Vec<Place>, Error> {
    let mut places: Vec<NominatimPlace> =
        serde_json::from_str(json).map_err(Error::UnexpectedData)?;
    places.retain(|place| place.place_rank > SMALLEST_REGION_RANK);
    // Best known first; the sort is stable, so ties keep Nominatim's order.
    places.sort_by(|a, b| score(b).total_cmp(&score(a)));
    places.into_iter().take(SHOWN).map(to_place).collect()
}

fn score(place: &NominatimPlace) -> f64 {
    let mountain = matches!(
        place.kind.as_str(),
        "peak"
            | "volcano"
            | "ridge"
            | "saddle"
            | "mountain_pass"
            | "glacier"
            | "winter_sports"
            | "alpine_hut"
    );
    if mountain {
        place.importance + MOUNTAIN_BONUS
    } else {
        place.importance
    }
}

fn to_place(place: NominatimPlace) -> Result<Place, Error> {
    Ok(Place {
        latitude: parse_degrees(&place.lat)?,
        longitude: parse_degrees(&place.lon)?,
        elevation: place
            .extratags
            .and_then(|tags| tags.ele)
            .and_then(|text| parse_elevation(&text)),
        name: with_settlement(
            name_or_start(place.name, &place.display_name),
            &place.addresstype,
            &place.address,
        ),
        kind: if place.addresstype.is_empty() {
            place.kind
        } else {
            place.addresstype
        },
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

// A neighbourhood or a postcode belongs to a town: "St. Gallen (Lachen)" says where
// it is better than "Lachen", which half a dozen places are called.
fn with_settlement(name: String, kind: &str, address: &Address) -> String {
    let part = matches!(
        kind,
        "suburb"
            | "quarter"
            | "neighbourhood"
            | "city_district"
            | "borough"
            | "postcode"
            | "locality"
            | "place"
    );
    let settlement = [&address.village, &address.town, &address.city]
        .into_iter()
        .flatten()
        .find(|settlement| **settlement != name);
    match settlement {
        Some(settlement) if part => format!("{settlement} ({name})"),
        Some(_) | None => name,
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
    fn leaves_out_whole_regions() {
        let places = parse(include_str!("../../tests/data/nominatim_st_gallen.json")).unwrap();

        let found: Vec<_> = places
            .iter()
            .map(|place| (place.name.as_str(), place.kind.as_str()))
            .collect();
        assert_eq!(found, [("St. Gallen", "city"), ("Sankt Gallen", "village")]);
    }

    #[test]
    fn neighbourhoods_carry_their_town() {
        let places = parse(include_str!("../../tests/data/nominatim_lachen.json")).unwrap();

        let names: Vec<_> = places.iter().map(|place| place.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "St. Gallen (Lachen)",
                "Wittenbach (Lachen)",
                "Betlis (Lachen)",
                "Henau (Lachen)"
            ]
        );
    }

    #[test]
    fn peaks_and_towns_keep_their_name() {
        let santis = parse(include_str!("../../tests/data/nominatim_santis.json")).unwrap();
        let st_gallen = parse(include_str!("../../tests/data/nominatim_st_gallen.json")).unwrap();

        assert_eq!(santis[0].name, "Säntis");
        assert_eq!(st_gallen[0].name, "St. Gallen");
    }

    #[test]
    fn best_known_comes_first() {
        let places = parse(include_str!("../../tests/data/nominatim_everest.json")).unwrap();

        assert_eq!(places.len(), 2);
        assert_eq!(places[0].name, "Mount Everest");
        assert_eq!(places[0].kind, "peak");
    }

    #[test]
    fn mountains_win_near_ties() {
        let place = |kind: &str, importance| NominatimPlace {
            lat: String::new(),
            lon: String::new(),
            name: String::new(),
            kind: kind.to_string(),
            importance,
            addresstype: String::new(),
            place_rank: 18,
            display_name: String::new(),
            address: Address::default(),
            extratags: None,
        };

        assert!(score(&place("peak", 0.40)) > score(&place("village", 0.45)));
        assert!(score(&place("peak", 0.05)) < score(&place("city", 0.60)));
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
