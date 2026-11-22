use crate::model::{Area, Meters, Point};

pub fn find(name: &str) -> Option<Area> {
    match name.to_lowercase().as_str() {
        "bodele" | "boedele" | "bödele" => Some(boedele()),
        _ => None,
    }
}

fn boedele() -> Area {
    Area {
        name: String::from("Bödele ski resort"),
        base: Point {
            latitude: 47.4178,
            longitude: 9.8182,
            elevation: Meters(1088.0),
        },
        summit: Point {
            latitude: 47.4031,
            longitude: 9.8132,
            elevation: Meters(1448.0),
        },
    }
}
