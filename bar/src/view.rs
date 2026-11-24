use slint::{ModelRc, SharedString, VecModel};

use crate::engine::{self, Place};
use crate::{Chart, Choice, Day, Legend, Mark, Spot, Tick, Tone};

/// The engine's panel in the types of the Slint panel.
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

// The engine works the panel out; here it only changes types, so that the Mac
// and Omarchy always show the same.
pub fn build(panel: &engine::Panel) -> View {
    View {
        daylight: panel.daylight.clone(),
        snow_line: panel.snow_line.clone(),
        freezing_level: panel.freezing_level.clone(),
        snow_depth: panel.snow_depth.clone(),
        spots: panel.spots.iter().map(spot).collect(),
        days: panel.days.iter().map(day).collect(),
        marks: panel.marks.iter().map(mark).collect(),
        temperature: chart(&panel.temperature),
        wind: chart(&panel.wind),
        precipitation: chart(&panel.precipitation),
        readings: panel
            .readings
            .iter()
            .map(|reading| reading.as_str().into())
            .collect(),
    }
}

fn spot(spot: &engine::Spot) -> Spot {
    Spot {
        label: spot.label.as_str().into(),
        height: spot.height.as_str().into(),
        sky: spot.sky.as_str().into(),
        temperature: spot.temperature.as_str().into(),
        wind: spot.wind.as_str().into(),
    }
}

fn day(day: &engine::Day) -> Day {
    Day {
        name: day.name.as_str().into(),
        sky: day.sky.as_str().into(),
        temperature: day.temperature.as_str().into(),
        wind: day.wind.as_str().into(),
        sun: day.sun.as_str().into(),
        snow: day.snow.as_str().into(),
    }
}

fn mark(mark: &engine::Mark) -> Mark {
    Mark {
        position: mark.position,
        text: mark.text.as_str().into(),
        midnight: mark.midnight,
    }
}

// Slint has no Option: -1 says that the chart has no line of 0 °C.
fn chart(chart: &engine::Chart) -> Chart {
    Chart {
        legend: model(chart.legend.iter().map(legend).collect()),
        line: chart.line.as_str().into(),
        snow: chart.snow.as_str().into(),
        rain: chart.rain.as_str().into(),
        ice: chart.ice.as_str().into(),
        ticks: model(chart.ticks.iter().map(tick).collect()),
        zero: chart.zero.unwrap_or(-1.0),
    }
}

fn legend(legend: &engine::Legend) -> Legend {
    Legend {
        text: legend.text.as_str().into(),
        tone: match legend.tone {
            engine::Tone::Line => Tone::Line,
            engine::Tone::Snow => Tone::Snow,
            engine::Tone::Ice => Tone::Ice,
        },
    }
}

fn tick(tick: &engine::Tick) -> Tick {
    Tick {
        position: tick.position,
        text: tick.text.as_str().into(),
    }
}

pub fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(items))
}

// Saved areas whose name contains what was typed, ignoring case.
pub fn matching(areas: &[String], typed: &str) -> Vec<usize> {
    let typed = typed.trim().to_lowercase();
    (0..areas.len())
        .filter(|&index| areas[index].to_lowercase().contains(&typed))
        .collect()
}

pub fn choices(areas: &[String], matches: &[usize], found: &[Place]) -> Vec<Choice> {
    // The last area stays, so it shows no ×.
    let removable = areas.len() > 1;
    let saved = matches.iter().map(|&index| Choice {
        title: areas[index].as_str().into(),
        detail: SharedString::new(),
        editable: true,
        removable,
    });
    let found = found.iter().map(|place| Choice {
        title: place.name.as_str().into(),
        detail: place.detail.as_str().into(),
        editable: false,
        removable: false,
    });
    saved.chain(found).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_filters_saved_areas_ignoring_case() {
        let areas = vec![
            String::from("Bödele ski resort"),
            String::from("Säntis"),
            String::from("Hoher Freschen"),
        ];

        assert_eq!(matching(&areas, ""), [0, 1, 2]);
        assert_eq!(matching(&areas, "säN"), [1]);
        assert_eq!(matching(&areas, " res"), [0, 2]);
        assert!(matching(&areas, "aneto").is_empty());
    }

    #[test]
    fn a_chart_without_zero_says_minus_one() {
        let chart = |zero| engine::Chart {
            legend: Vec::new(),
            line: String::new(),
            snow: String::new(),
            rain: String::new(),
            ice: String::new(),
            ticks: Vec::new(),
            zero,
        };

        assert_eq!(super::chart(&chart(Some(0.5))).zero, 0.5);
        assert_eq!(super::chart(&chart(None)).zero, -1.0);
    }
}
