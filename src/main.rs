mod model;

use crate::model::{Area, Meters, Point};

fn main() {
    let boedele = Area {
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
    };

    println!("{}", boedele.name);
    for (label, point) in [("Base", boedele.base), ("Summit", boedele.summit)] {
        println!(
            "{label}: {} m ({}, {})",
            point.elevation.0, point.latitude, point.longitude
        );
    }
}
