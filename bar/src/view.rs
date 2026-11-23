use slint::{ModelRc, SharedString, VecModel};

use crate::chart::{self, Scale};
use crate::clock;
use crate::engine::{Daylight, Fall, Forecast, Hour, Report, Sky};
use crate::{Chart, Day, Legend, Mark, Spot, Tick, Tone};

const CHART_HOURS: usize = 48;
// Below this, a day counts as dry.
const WET_DAY_MM: f64 = 1.0;

pub struct View {
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
    pub readings: Vec<SharedString>,
}

pub fn build(report: &Report, now: i64) -> View {
    let summit = &report.summit;
    let offset = summit.utc_offset_seconds;
    let current = current_hour(&summit.hours, now);
    let hours: Vec<&Hour> = summit
        .hours
        .iter()
        .skip_while(|hour| current.is_some_and(|current| hour.time < current.time))
        .take(CHART_HOURS)
        .collect();

    let mut now_spots = vec![spot("summit", summit, now)];
    now_spots.extend(report.base.as_ref().map(|base| spot("base", base, now)));
    let totals: Vec<Totals> = summit
        .hours
        .chunk_by(|a, b| clock::day(a.time, offset) == clock::day(b.time, offset))
        .map(|hours| totals(hours, &summit.days, offset))
        .collect();

    View {
        daylight: daylight(summit, now),
        snow_line: meters(current.and_then(|hour| hour.snow_line_m)),
        freezing_level: meters(current.map(|hour| hour.freezing_level_m)),
        snow_depth: current.map_or(dash(), |hour| {
            format!("{} cm", whole(hour.snow_depth_m * 100.0))
        }),
        spots: spots(&now_spots),
        days: days(&totals),
        marks: marks(&hours, offset),
        temperature: temperature_chart(&hours),
        wind: wind_chart(&hours),
        precipitation: precipitation_chart(&hours),
        readings: hours
            .iter()
            .map(|hour| reading(hour, offset).into())
            .collect(),
    }
}

pub fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(items))
}

fn current_hour(hours: &[Hour], now: i64) -> Option<&Hour> {
    hours.iter().rfind(|hour| hour.time <= now)
}

// What one spot shows now, before the columns are lined up.
struct Now {
    label: &'static str,
    height: String,
    sky: &'static str,
    temperature: String,
    wind: String,
}

fn spot(label: &'static str, forecast: &Forecast, now: i64) -> Now {
    let hour = current_hour(&forecast.hours, now);
    Now {
        label,
        height: whole(forecast.point.elevation),
        sky: hour.map_or("-", sky_label),
        temperature: hour.map_or(dash(), |hour| whole(hour.temperature_c)),
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
            label: now.label.into(),
            height: format!("{:>height$} m", now.height).into(),
            sky: now.sky.into(),
            temperature: format!("{:>temperature$} °C", now.temperature).into(),
            wind: format!("{:>wind$} km/h", now.wind).into(),
        })
        .collect()
}

fn daylight(forecast: &Forecast, now: i64) -> String {
    let offset = forecast.utc_offset_seconds;
    forecast
        .days
        .iter()
        .find(|day| clock::day(day.sunrise, offset) == clock::day(now, offset))
        .map_or(String::new(), |day| {
            let minutes = (day.sunset - day.sunrise) / 60;
            format!(
                "{} - {}  ({} h {:02})",
                clock::clock(day.sunrise, offset),
                clock::clock(day.sunset, offset),
                minutes / 60,
                minutes % 60
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

fn totals(hours: &[Hour], days: &[Daylight], offset: i64) -> Totals {
    let start = hours.first().map_or(0, |hour| hour.time);
    let light = days
        .iter()
        .find(|day| clock::day(day.sunrise, offset) == clock::day(start, offset));
    let low = hours
        .iter()
        .map(|hour| hour.temperature_c)
        .fold(f64::INFINITY, f64::min);
    let high = hours
        .iter()
        .map(|hour| hour.temperature_c)
        .fold(f64::NEG_INFINITY, f64::max);
    let wind = hours.iter().map(Hour::best_wind).fold(0.0, f64::max);
    let sun: f64 = hours.iter().map(|hour| hour.sunshine_minutes).sum();
    let snow: f64 = hours.iter().map(|hour| hour.fresh_snow_cm).sum();
    Totals {
        name: clock::weekday(start, offset),
        sky: day_sky(hours, light),
        low: whole(low),
        high: whole(high),
        wind: whole(wind),
        sun: format!("{:.1}", sun / 60.0),
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
            name: day.name.into(),
            sky: day.sky.into(),
            temperature: format!("{:>low$} / {:>high$}", day.low, day.high).into(),
            wind: format!("{:>wind$} km/h", day.wind).into(),
            sun: format!("{:>sun$} h", day.sun).into(),
            snow: format!("{:>snow$} cm", day.snow).into(),
        })
        .collect()
}

// The panel font is monospaced: padding each number in a column to the longest one
// lines up the last digits and the units, whatever the sign.
fn width<'a>(texts: impl Iterator<Item = &'a str>) -> usize {
    texts.map(|text| text.chars().count()).max().unwrap_or(0)
}

fn sky_label(hour: &Hour) -> &'static str {
    match hour.fall() {
        Fall::Nothing => hour.sky.label(),
        Fall::Freezing => "icy rain",
        Fall::Snow if matches!(hour.sky, Sky::SnowShowers | Sky::Thunderstorm) => hour.sky.label(),
        Fall::Snow => "snow",
        Fall::Rain if matches!(hour.sky, Sky::Snow | Sky::SnowShowers) => "rain",
        Fall::Rain => hour.sky.label(),
    }
}

// Only the daylight hours count, and a little rain or snow does not make a day rainy or snowy.
fn day_sky(hours: &[Hour], light: Option<&Daylight>) -> &'static str {
    let hours: Vec<&Hour> = hours
        .iter()
        .filter(|hour| {
            light.is_none_or(|day| hour.time + 3600 > day.sunrise && hour.time < day.sunset)
        })
        .collect();
    let total = |kind: Fall| amounts(hours.iter().copied(), kind).iter().sum::<f64>();
    let (snow, ice, rain) = (total(Fall::Snow), total(Fall::Freezing), total(Fall::Rain));
    let wet = snow + ice + rain;
    let other = if ice > rain { "ice" } else { "rain" };
    if wet < WET_DAY_MM {
        dry_sky(&hours).map_or("cloudy", Sky::label)
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
fn dry_sky(hours: &[&Hour]) -> Option<Sky> {
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

fn amounts<'a>(hours: impl Iterator<Item = &'a Hour>, kind: Fall) -> Vec<f64> {
    hours
        .map(|hour| {
            if hour.fall() == kind {
                hour.precipitation_mm
            } else {
                0.0
            }
        })
        .collect()
}

fn marks(hours: &[&Hour], offset: i64) -> Vec<Mark> {
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
                    clock::weekday(hour.time, offset).into()
                } else {
                    format!("{local:02}h").into()
                },
                midnight,
            }
        })
        .collect()
}

fn temperature_chart(hours: &[&Hour]) -> Chart {
    let values: Vec<f64> = hours.iter().map(|hour| hour.temperature_c).collect();
    let scale = Scale::around(&values);
    chart(
        &[("temperature °C", Tone::Line)],
        scale,
        chart::line(&values, scale),
    )
}

fn wind_chart(hours: &[&Hour]) -> Chart {
    let values: Vec<f64> = hours.iter().map(|hour| hour.best_wind()).collect();
    let scale = Scale::from_zero(&values, 10.0);
    chart(
        &[("wind km/h", Tone::Line)],
        scale,
        chart::line(&values, scale),
    )
}

fn precipitation_chart(hours: &[&Hour]) -> Chart {
    let all: Vec<f64> = hours.iter().map(|hour| hour.precipitation_mm).collect();
    let part = |kind: Fall| amounts(hours.iter().copied(), kind);
    let scale = Scale::from_zero(&all, 1.0);
    Chart {
        snow: chart::bars(&part(Fall::Snow), scale).into(),
        rain: chart::bars(&part(Fall::Rain), scale).into(),
        ice: chart::bars(&part(Fall::Freezing), scale).into(),
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
    let ticks = scale
        .ticks()
        .into_iter()
        .map(|value| Tick {
            position: scale.position(value) as f32,
            text: whole(value).into(),
        })
        .collect();
    Chart {
        legend: model(
            legend
                .iter()
                .map(|(text, tone)| Legend {
                    text: (*text).into(),
                    tone: *tone,
                })
                .collect(),
        ),
        line: line.into(),
        snow: SharedString::new(),
        rain: SharedString::new(),
        ice: SharedString::new(),
        ticks: model(ticks),
    }
}

fn reading(hour: &Hour, offset: i64) -> String {
    let fall = match hour.fall() {
        Fall::Nothing => String::from("dry"),
        Fall::Rain => format!("rain {:.1} mm", hour.precipitation_mm),
        Fall::Freezing => format!("freezing rain {:.1} mm", hour.precipitation_mm),
        Fall::Snow => format!("snow {:.1} mm", hour.precipitation_mm),
    };
    format!(
        "{} {}  {} °C  wind {} km/h  {fall}",
        clock::weekday(hour.time, offset),
        clock::clock(hour.time, offset),
        whole(hour.temperature_c),
        whole(hour.best_wind())
    )
}

fn fraction(index: usize, total: usize) -> f32 {
    (index as f32 + 0.5) / total.max(1) as f32
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
    use slint::Model;

    use super::*;
    use crate::engine::Point;

    // Midnight of Sunday 22 November 2026 in Vienna.
    const START: i64 = 1795302000;

    fn hour(index: i64) -> Hour {
        Hour {
            time: START + index * 3600,
            sky: if index < 24 { Sky::Snow } else { Sky::Clear },
            temperature_c: -0.2,
            precipitation_mm: if index == 14 { 1.5 } else { 0.0 },
            fresh_snow_cm: if index == 14 { 1.2 } else { 0.0 },
            snow_depth_m: 0.14,
            wind_kmh: 5.0,
            high_wind_kmh: (index != 3).then_some(30.0),
            sunshine_minutes: 0.0,
            freezing_level_m: 1390.0,
            snow_line_m: None,
        }
    }

    fn report() -> Report {
        Report {
            summit: Forecast {
                point: Point { elevation: 1448.0 },
                utc_offset_seconds: 3600,
                hours: (0..72).map(hour).collect(),
                days: vec![Daylight {
                    sunrise: START + 27300,
                    sunset: START + 59880,
                }],
            },
            base: None,
        }
    }

    #[test]
    fn shows_the_current_hour() {
        let view = build(&report(), START + 14 * 3600 + 1800);

        assert_eq!(view.spots.len(), 1);
        assert_eq!(view.spots[0].temperature, "0 °C");
        assert_eq!(view.spots[0].wind, "30 km/h");
        assert_eq!(view.snow_line, "-");
        assert_eq!(view.snow_depth, "14 cm");
        assert_eq!(view.daylight, "07:35 - 16:38  (9 h 03)");
        assert_eq!(
            view.readings[0],
            "Sun 14:00  0 °C  wind 30 km/h  snow 1.5 mm"
        );
    }

    #[test]
    fn charts_start_now_and_have_named_hours() {
        let view = build(&report(), START + 14 * 3600);

        assert_eq!(view.readings.len(), 48);
        let texts: Vec<_> = view.marks.iter().map(|mark| mark.text.as_str()).collect();
        assert_eq!(
            texts,
            ["18h", "Mon", "06h", "12h", "18h", "Tue", "06h", "12h"]
        );
        assert!(view.precipitation.rain.is_empty());
        assert!(!view.precipitation.snow.is_empty());
        assert_eq!(view.temperature.ticks.row_count(), 2);
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
        let view = build(&report(), START);

        assert_eq!(view.days.len(), 3);
        assert_eq!(view.days[0].name, "Sun");
        assert_eq!(view.days[0].sky, "snow");
        assert_eq!(view.days[0].snow, "1 cm");
        assert_eq!(view.days[1].sky, "clear");
    }

    #[test]
    fn a_little_snow_is_still_a_cloudy_day() {
        let mut report = report();
        for hour in &mut report.summit.hours {
            hour.sky = Sky::Cloudy;
            hour.precipitation_mm = 0.0;
            hour.fresh_snow_cm = 0.0;
        }
        report.summit.hours[10].sky = Sky::Snow;
        report.summit.hours[10].precipitation_mm = 0.4;
        report.summit.hours[10].fresh_snow_cm = 0.28;

        assert_eq!(build(&report, START).days[0].sky, "cloudy");
    }

    #[test]
    fn a_rainy_day_with_some_snow_is_rainy() {
        let mut report = report();
        for hour in &mut report.summit.hours[0..24] {
            hour.sky = Sky::Rain;
            hour.precipitation_mm = 0.5;
            hour.fresh_snow_cm = 0.0;
        }
        report.summit.hours[10].sky = Sky::Snow;
        report.summit.hours[10].fresh_snow_cm = 0.35;

        assert_eq!(build(&report, START).days[0].sky, "rain");
    }

    #[test]
    fn what_falls_at_night_does_not_change_the_day() {
        let mut report = report();
        for hour in &mut report.summit.hours[0..24] {
            hour.sky = Sky::Fog;
            hour.precipitation_mm = 0.0;
            hour.fresh_snow_cm = 0.0;
        }
        for hour in &mut report.summit.hours[0..6] {
            hour.sky = Sky::Snow;
            hour.precipitation_mm = 1.0;
            hour.fresh_snow_cm = 0.7;
        }

        let day = &build(&report, START).days[0];

        assert_eq!(day.sky, "fog");
        assert_eq!(day.snow, "4 cm");
    }

    #[test]
    fn freezing_rain_has_its_own_bars() {
        let mut report = report();
        for hour in &mut report.summit.hours[20..22] {
            hour.sky = Sky::FreezingDrizzle;
            hour.precipitation_mm = 0.4;
            hour.fresh_snow_cm = 0.0;
        }

        let view = build(&report, START);

        assert!(!view.precipitation.ice.is_empty());
        assert!(build(&self::report(), START).precipitation.ice.is_empty());
    }

    #[test]
    fn old_data_does_not_fail() {
        let view = build(&report(), START + 30 * 86_400);

        assert_eq!(view.readings.len(), 1);
        assert_eq!(view.daylight, "");
    }
}
