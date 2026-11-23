// What the panels show, already worked out: they only draw it. Numbers come as
// text with their units, padded so that columns line up in a monospaced font.

mod chart;

use serde::Serialize;

use crate::clock;
use crate::model::{Daylight, Fall, Forecast, HourlyForecast, Sky, UnixTime};
use crate::output::Report;
use chart::Scale;

const CHART_HOURS: usize = 48;
// Below this, a day counts as dry.
const WET_DAY_MM: f64 = 1.0;

#[derive(Debug, Serialize)]
pub struct Panel {
    pub name: String,
    pub daylight: String,
    pub snow_line: String,
    pub freezing_level: String,
    pub snow_depth: String,
    pub spots: Vec<Spot>,
    pub days: Vec<Day>,
    pub marks: Vec<Mark>,
    pub chart_width: f64,
    pub chart_height: f64,
    pub temperature: Chart,
    pub wind: Chart,
    pub precipitation: Chart,
    /// One line per hour of the charts, for the hour under the pointer.
    pub readings: Vec<String>,
    pub credits: &'static str,
}

/// The summit or the base now.
#[derive(Debug, Serialize)]
pub struct Spot {
    pub label: String,
    pub height: String,
    pub sky: String,
    pub temperature: String,
    pub wind: String,
}

#[derive(Debug, Serialize)]
pub struct Day {
    pub name: String,
    pub sky: String,
    pub temperature: String,
    pub wind: String,
    pub sun: String,
    pub snow: String,
}

/// A line across the charts every six hours.
#[derive(Debug, Serialize)]
pub struct Mark {
    /// From 0 at the left of the charts to 1 at the right.
    pub position: f64,
    pub text: String,
    pub midnight: bool,
}

/// Paths in SVG syntax, in a box of `chart_width` by `chart_height`. An empty one
/// draws nothing.
#[derive(Debug, Serialize)]
pub struct Chart {
    pub legend: Vec<Legend>,
    pub line: String,
    pub snow: String,
    pub rain: String,
    pub ice: String,
    pub ticks: Vec<Tick>,
    /// Where the line of 0 °C is, from 0 at the top to 1 at the bottom; only in the
    /// temperature chart.
    pub zero: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct Legend {
    pub text: String,
    pub tone: Tone,
}

/// The colour of a legend: the one of the chart, or the one of snow or ice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Line,
    Snow,
    Ice,
}

#[derive(Debug, Serialize)]
pub struct Tick {
    /// From 0 at the top of the chart to 1 at the bottom.
    pub position: f64,
    pub text: String,
}

pub fn build(report: &Report, now: UnixTime) -> Panel {
    let summit = &report.summit;
    let offset = summit.utc_offset_seconds;
    let current = current_hour(&summit.hours, now);
    let hours: Vec<&HourlyForecast> = summit
        .hours
        .iter()
        .skip_while(|hour| current.is_some_and(|current| hour.time.0 < current.time.0))
        .take(CHART_HOURS)
        .collect();

    let mut now_spots = vec![spot(&point_label(report), summit, now)];
    now_spots.extend(report.base.as_ref().map(|base| spot("base", base, now)));
    let totals: Vec<Totals> = summit
        .hours
        .chunk_by(|a, b| clock::day(a.time, offset) == clock::day(b.time, offset))
        .map(|hours| totals(hours, &summit.days, offset))
        .collect();

    Panel {
        name: report.name.clone(),
        daylight: daylight(summit, now),
        snow_line: meters(current.and_then(|hour| hour.snow_line.map(|line| line.0))),
        freezing_level: meters(current.map(|hour| hour.freezing_level.0)),
        snow_depth: current.map_or(dash(), |hour| {
            format!("{} cm", whole(hour.snow_depth.0 * 100.0))
        }),
        spots: spots(&now_spots),
        days: days(&totals),
        marks: marks(&hours, offset),
        chart_width: chart::WIDTH,
        chart_height: chart::HEIGHT,
        temperature: temperature_chart(&hours),
        wind: wind_chart(&hours),
        precipitation: precipitation_chart(&hours),
        readings: hours.iter().map(|hour| reading(hour, offset)).collect(),
        credits: report.credits,
    }
}

fn current_hour(hours: &[HourlyForecast], now: UnixTime) -> Option<&HourlyForecast> {
    hours.iter().rfind(|hour| hour.time.0 <= now.0)
}

// "summit" only makes sense with a base below it; a single point says what it is.
fn point_label(report: &Report) -> String {
    match (&report.base, &report.kind) {
        (Some(_), _) => String::from("summit"),
        (None, Some(kind)) => kind.replace('_', " "),
        (None, None) => String::from("elevation"),
    }
}

// What one spot shows now, before the columns are lined up.
struct Now {
    label: String,
    height: String,
    sky: &'static str,
    temperature: String,
    wind: String,
}

fn spot(label: &str, forecast: &Forecast, now: UnixTime) -> Now {
    let hour = current_hour(&forecast.hours, now);
    Now {
        label: label.to_string(),
        height: whole(forecast.point.elevation.0),
        sky: hour.map_or("-", sky_label),
        temperature: hour.map_or(dash(), |hour| whole(hour.temperature.0)),
        wind: hour.map_or(dash(), |hour| whole(hour.best_wind())),
    }
}

fn spots(readings: &[Now]) -> Vec<Spot> {
    let height = width(readings.iter().map(|now| now.height.as_str()));
    let temperature = width(readings.iter().map(|now| now.temperature.as_str()));
    let wind = width(readings.iter().map(|now| now.wind.as_str()));
    readings
        .iter()
        .map(|now| Spot {
            label: now.label.clone(),
            height: format!("{:>height$} m", now.height),
            sky: now.sky.to_string(),
            temperature: format!("{:>temperature$} °C", now.temperature),
            wind: format!("{:>wind$} km/h", now.wind),
        })
        .collect()
}

fn daylight(forecast: &Forecast, now: UnixTime) -> String {
    let offset = forecast.utc_offset_seconds;
    forecast
        .days
        .iter()
        .find(|day| clock::day(day.sunrise, offset) == clock::day(now, offset))
        .map_or(String::new(), |day| {
            format!(
                "{} - {}",
                clock::clock(day.sunrise, offset),
                clock::clock(day.sunset, offset)
            )
        })
}

// One day of the table, before the columns are lined up.
struct Totals {
    name: &'static str,
    sky: &'static str,
    low: String,
    high: String,
    wind: String,
    sun: String,
    snow: String,
}

fn totals(hours: &[HourlyForecast], days: &[Daylight], offset: i32) -> Totals {
    let start = hours.first().map_or(UnixTime(0), |hour| hour.time);
    let light = days
        .iter()
        .find(|day| clock::day(day.sunrise, offset) == clock::day(start, offset));
    let low = hours
        .iter()
        .map(|hour| hour.temperature.0)
        .fold(f64::INFINITY, f64::min);
    let high = hours
        .iter()
        .map(|hour| hour.temperature.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let wind = hours
        .iter()
        .map(HourlyForecast::best_wind)
        .fold(0.0, f64::max);
    let sun: f64 = hours.iter().map(|hour| hour.sunshine.as_secs_f64()).sum();
    let snow: f64 = hours.iter().map(|hour| hour.fresh_snow.0).sum();
    Totals {
        name: clock::weekday(start, offset),
        sky: day_sky(hours, light),
        low: whole(low),
        high: whole(high),
        wind: whole(wind),
        sun: format!("{:.1}", sun / 3600.0),
        snow: whole(snow),
    }
}

fn days(totals: &[Totals]) -> Vec<Day> {
    let low = width(totals.iter().map(|day| day.low.as_str()));
    let high = width(totals.iter().map(|day| day.high.as_str()));
    let wind = width(totals.iter().map(|day| day.wind.as_str()));
    let sun = width(totals.iter().map(|day| day.sun.as_str()));
    let snow = width(totals.iter().map(|day| day.snow.as_str()));
    totals
        .iter()
        .map(|day| Day {
            name: day.name.to_string(),
            sky: day.sky.to_string(),
            temperature: format!("{:>low$} / {:>high$}", day.low, day.high),
            wind: format!("{:>wind$} km/h", day.wind),
            sun: format!("{:>sun$} h", day.sun),
            snow: format!("{:>snow$} cm", day.snow),
        })
        .collect()
}

// The panel font is monospaced: padding each number in a column to the longest one
// lines up the last digits and the units, whatever the sign.
fn width<'a>(texts: impl Iterator<Item = &'a str>) -> usize {
    texts.map(|text| text.chars().count()).max().unwrap_or(0)
}

fn sky_name(sky: Sky) -> &'static str {
    match sky {
        Sky::Clear => "clear",
        Sky::Cloudy => "cloudy",
        Sky::Fog => "fog",
        Sky::Drizzle => "drizzle",
        Sky::FreezingDrizzle => "icy drizzle",
        Sky::Rain => "rain",
        Sky::FreezingRain => "icy rain",
        Sky::Snow => "snow",
        Sky::RainShowers => "rain showers",
        Sky::SnowShowers => "snow showers",
        Sky::Thunderstorm => "thunderstorm",
    }
}

fn sky_label(hour: &HourlyForecast) -> &'static str {
    match hour.fall() {
        Fall::Nothing => sky_name(hour.sky),
        Fall::Freezing => "icy rain",
        Fall::Snow if matches!(hour.sky, Sky::SnowShowers | Sky::Thunderstorm) => {
            sky_name(hour.sky)
        }
        Fall::Snow => "snow",
        Fall::Rain if matches!(hour.sky, Sky::Snow | Sky::SnowShowers) => "rain",
        Fall::Rain => sky_name(hour.sky),
    }
}

// Only the daylight hours count, and a little rain or snow does not make a day rainy or snowy.
fn day_sky(hours: &[HourlyForecast], light: Option<&Daylight>) -> &'static str {
    let hours: Vec<&HourlyForecast> = hours
        .iter()
        .filter(|hour| {
            light.is_none_or(|day| hour.time.0 + 3600 > day.sunrise.0 && hour.time.0 < day.sunset.0)
        })
        .collect();
    let total = |kind: Fall| amounts(hours.iter().copied(), kind).iter().sum::<f64>();
    let (snow, ice, rain) = (total(Fall::Snow), total(Fall::Freezing), total(Fall::Rain));
    let wet = snow + ice + rain;
    let other = if ice > rain { "ice" } else { "rain" };
    if wet < WET_DAY_MM {
        dry_sky(&hours).map_or("cloudy", sky_name)
    } else if snow >= wet * 2.0 / 3.0 {
        "snow"
    } else if snow <= wet / 3.0 {
        other
    } else if other == "ice" {
        "snow, ice"
    } else {
        "snow, rain"
    }
}

// The most usual one; between two equals, the worse one.
fn dry_sky(hours: &[&HourlyForecast]) -> Option<Sky> {
    let skies: Vec<Sky> = hours
        .iter()
        .map(|hour| hour.sky)
        .filter(|sky| matches!(sky, Sky::Clear | Sky::Cloudy | Sky::Fog))
        .collect();
    skies
        .iter()
        .copied()
        .max_by_key(|sky| (skies.iter().filter(|other| *other == sky).count(), *sky))
}

fn amounts<'a>(hours: impl Iterator<Item = &'a HourlyForecast>, kind: Fall) -> Vec<f64> {
    hours
        .map(|hour| {
            if hour.fall() == kind {
                hour.precipitation.0
            } else {
                0.0
            }
        })
        .collect()
}

fn marks(hours: &[&HourlyForecast], offset: i32) -> Vec<Mark> {
    hours
        .iter()
        .enumerate()
        .filter(|(_, hour)| clock::hour(hour.time, offset) % 6 == 0)
        .map(|(index, hour)| {
            let local = clock::hour(hour.time, offset);
            let midnight = local == 0;
            Mark {
                position: fraction(index, hours.len()),
                text: if midnight {
                    clock::weekday(hour.time, offset).to_string()
                } else {
                    format!("{local:02}h")
                },
                midnight,
            }
        })
        .collect()
}

fn temperature_chart(hours: &[&HourlyForecast]) -> Chart {
    let values: Vec<f64> = hours.iter().map(|hour| hour.temperature.0).collect();
    let scale = Scale::temperature(&values);
    Chart {
        zero: Some(scale.position(0.0)),
        ..chart(
            &[("temperature °C", Tone::Line)],
            scale,
            chart::line(&values, scale),
        )
    }
}

fn wind_chart(hours: &[&HourlyForecast]) -> Chart {
    let values: Vec<f64> = hours.iter().map(|hour| hour.best_wind()).collect();
    let scale = Scale::from_zero(&values, 10.0);
    chart(
        &[("wind km/h", Tone::Line)],
        scale,
        chart::line(&values, scale),
    )
}

fn precipitation_chart(hours: &[&HourlyForecast]) -> Chart {
    let all: Vec<f64> = hours.iter().map(|hour| hour.precipitation.0).collect();
    let part = |kind: Fall| amounts(hours.iter().copied(), kind);
    let scale = Scale::from_zero(&all, 1.0);
    Chart {
        snow: chart::bars(&part(Fall::Snow), scale),
        rain: chart::bars(&part(Fall::Rain), scale),
        ice: chart::bars(&part(Fall::Freezing), scale),
        ..chart(
            &[
                ("rain", Tone::Line),
                ("snow", Tone::Snow),
                ("ice", Tone::Ice),
                ("mm", Tone::Line),
            ],
            scale,
            String::new(),
        )
    }
}

fn chart(legend: &[(&str, Tone)], scale: Scale, line: String) -> Chart {
    Chart {
        legend: legend
            .iter()
            .map(|(text, tone)| Legend {
                text: (*text).to_string(),
                tone: *tone,
            })
            .collect(),
        line,
        snow: String::new(),
        rain: String::new(),
        ice: String::new(),
        ticks: scale
            .ticks()
            .into_iter()
            .map(|value| Tick {
                position: scale.position(value),
                text: whole(value),
            })
            .collect(),
        zero: None,
    }
}

fn reading(hour: &HourlyForecast, offset: i32) -> String {
    let fall = match hour.fall() {
        Fall::Nothing => String::from("dry"),
        Fall::Rain => format!("rain {:.1} mm", hour.precipitation.0),
        Fall::Freezing => format!("freezing rain {:.1} mm", hour.precipitation.0),
        Fall::Snow => format!("snow {:.1} mm", hour.precipitation.0),
    };
    format!(
        "{} {}  {} °C  wind {} km/h  {fall}",
        clock::weekday(hour.time, offset),
        clock::clock(hour.time, offset),
        whole(hour.temperature.0),
        whole(hour.best_wind())
    )
}

fn fraction(index: usize, total: usize) -> f64 {
    (count(index) + 0.5) / count(total.max(1))
}

fn count(number: usize) -> f64 {
    u32::try_from(number).map_or(0.0, f64::from)
}

// Avoids "-0".
fn whole(value: f64) -> String {
    format!("{:.0}", value.round() + 0.0)
}

fn meters(value: Option<f64>) -> String {
    value.map_or(dash(), |value| format!("{} m", whole(value)))
}

fn dash() -> String {
    String::from("-")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::model::{Celsius, Centimeters, KilometersPerHour, Meters, Millimeters, Point};
    use crate::output::WEATHER_CREDIT;

    // Midnight of Sunday 22 November 2026 in Vienna.
    const START: i64 = 1795302000;

    fn at(seconds: i64) -> UnixTime {
        UnixTime(START + seconds)
    }

    fn hour(index: i64) -> HourlyForecast {
        HourlyForecast {
            time: at(index * 3600),
            sky: if index < 24 { Sky::Snow } else { Sky::Clear },
            temperature: Celsius(-0.2),
            precipitation: Millimeters(if index == 14 { 1.5 } else { 0.0 }),
            fresh_snow: Centimeters(if index == 14 { 1.2 } else { 0.0 }),
            snow_depth: Meters(0.14),
            wind: KilometersPerHour(5.0),
            gusts: KilometersPerHour(9.0),
            high_wind: (index != 3).then_some(KilometersPerHour(30.0)),
            visibility: Meters(20000.0),
            sunshine: Duration::ZERO,
            freezing_level: Meters(1390.0),
            snow_line: None,
            precipitation_probability: None,
        }
    }

    fn report() -> Report {
        Report {
            name: String::from("Bödele"),
            kind: None,
            summit: Forecast {
                point: Point {
                    latitude: 47.4031,
                    longitude: 9.8132,
                    elevation: Meters(1448.0),
                },
                utc_offset_seconds: 3600,
                hours: (0..72).map(hour).collect(),
                days: vec![Daylight {
                    sunrise: at(27300),
                    sunset: at(59880),
                }],
            },
            base: None,
            credits: WEATHER_CREDIT,
        }
    }

    #[test]
    fn summit_only_above_a_base() {
        let mut report = report();
        assert_eq!(point_label(&report), "elevation");

        report.kind = Some(String::from("city_district"));
        assert_eq!(point_label(&report), "city district");

        report.base = Some(report.summit.clone());
        assert_eq!(point_label(&report), "summit");
    }

    #[test]
    fn shows_the_current_hour() {
        let panel = build(&report(), at(14 * 3600 + 1800));

        assert_eq!(panel.spots.len(), 1);
        assert_eq!(panel.spots[0].temperature, "0 °C");
        assert_eq!(panel.spots[0].wind, "30 km/h");
        assert_eq!(panel.snow_line, "-");
        assert_eq!(panel.snow_depth, "14 cm");
        assert_eq!(panel.daylight, "07:35 - 16:38");
        assert_eq!(
            panel.readings[0],
            "Sun 14:00  0 °C  wind 30 km/h  snow 1.5 mm"
        );
    }

    #[test]
    fn charts_start_now_and_have_named_hours() {
        let panel = build(&report(), at(14 * 3600));

        assert_eq!(panel.readings.len(), 48);
        let texts: Vec<_> = panel.marks.iter().map(|mark| mark.text.as_str()).collect();
        assert_eq!(
            texts,
            ["18h", "Mon", "06h", "12h", "18h", "Tue", "06h", "12h"]
        );
        assert!(panel.precipitation.rain.is_empty());
        assert!(!panel.precipitation.snow.is_empty());
        assert_eq!(panel.temperature.ticks.len(), 5);
        assert_eq!(panel.temperature.zero, Some(0.25));
        assert_eq!(panel.wind.zero, None);
    }

    #[test]
    fn columns_line_up_whatever_the_sign() {
        let day = |low: &str, high: &str, wind: &str| Totals {
            name: "Mon",
            sky: "snow",
            low: low.into(),
            high: high.into(),
            wind: wind.into(),
            sun: "0.0".into(),
            snow: "0".into(),
        };

        let days = days(&[day("-4", "-2", "19"), day("-3", "1", "9")]);

        assert_eq!(days[0].temperature, "-4 / -2");
        assert_eq!(days[1].temperature, "-3 /  1");
        assert_eq!(days[0].wind, "19 km/h");
        assert_eq!(days[1].wind, " 9 km/h");
    }

    #[test]
    fn days_are_summed() {
        let panel = build(&report(), at(0));

        assert_eq!(panel.days.len(), 3);
        assert_eq!(panel.days[0].name, "Sun");
        assert_eq!(panel.days[0].sky, "snow");
        assert_eq!(panel.days[0].snow, "1 cm");
        assert_eq!(panel.days[1].sky, "clear");
    }

    #[test]
    fn a_little_snow_is_still_a_cloudy_day() {
        let mut report = report();
        for hour in &mut report.summit.hours {
            hour.sky = Sky::Cloudy;
            hour.precipitation = Millimeters(0.0);
            hour.fresh_snow = Centimeters(0.0);
        }
        report.summit.hours[10].sky = Sky::Snow;
        report.summit.hours[10].precipitation = Millimeters(0.4);
        report.summit.hours[10].fresh_snow = Centimeters(0.28);

        assert_eq!(build(&report, at(0)).days[0].sky, "cloudy");
    }

    #[test]
    fn a_rainy_day_with_some_snow_is_rainy() {
        let mut report = report();
        for hour in &mut report.summit.hours[0..24] {
            hour.sky = Sky::Rain;
            hour.precipitation = Millimeters(0.5);
            hour.fresh_snow = Centimeters(0.0);
        }
        report.summit.hours[10].sky = Sky::Snow;
        report.summit.hours[10].fresh_snow = Centimeters(0.35);

        assert_eq!(build(&report, at(0)).days[0].sky, "rain");
    }

    #[test]
    fn what_falls_at_night_does_not_change_the_day() {
        let mut report = report();
        for hour in &mut report.summit.hours[0..24] {
            hour.sky = Sky::Fog;
            hour.precipitation = Millimeters(0.0);
            hour.fresh_snow = Centimeters(0.0);
        }
        for hour in &mut report.summit.hours[0..6] {
            hour.sky = Sky::Snow;
            hour.precipitation = Millimeters(1.0);
            hour.fresh_snow = Centimeters(0.7);
        }

        let day = &build(&report, at(0)).days[0];

        assert_eq!(day.sky, "fog");
        assert_eq!(day.snow, "4 cm");
    }

    #[test]
    fn freezing_rain_has_its_own_bars() {
        let mut report = report();
        for hour in &mut report.summit.hours[20..22] {
            hour.sky = Sky::FreezingDrizzle;
            hour.precipitation = Millimeters(0.4);
            hour.fresh_snow = Centimeters(0.0);
        }

        assert!(!build(&report, at(0)).precipitation.ice.is_empty());
        assert!(build(&self::report(), at(0)).precipitation.ice.is_empty());
    }

    #[test]
    fn old_data_does_not_fail() {
        let panel = build(&report(), at(30 * 86_400));

        assert_eq!(panel.readings.len(), 1);
        assert_eq!(panel.daylight, "");
    }

    #[test]
    fn json_names_tones_in_lower_case() {
        let json = serde_json::to_value(build(&report(), at(0))).unwrap();

        assert_eq!(json["precipitation"]["legend"][1]["tone"], "snow");
        assert_eq!(json["chart_width"], 370.0);
        assert_eq!(json["credits"], WEATHER_CREDIT);
    }
}
