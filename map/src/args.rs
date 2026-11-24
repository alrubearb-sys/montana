pub const USAGE: &str = "usage: peakcast-map <latitude,longitude of where to start>
       peakcast-map --edit <area> <summit latitude,longitude> [<base latitude,longitude>]";

pub enum Args {
    Create {
        center: (f64, f64),
    },
    Edit {
        area: String,
        summit: (f64, f64),
        base: Option<(f64, f64)>,
    },
}

pub fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let first = args.next().ok_or("missing where to start")?;
    let read = if first == "--edit" {
        let area = args.next().ok_or("missing the area")?;
        let summit = args.next().ok_or("missing the summit")?;
        Args::Edit {
            area,
            summit: coordinates(&summit)?,
            base: args.next().map(|base| coordinates(&base)).transpose()?,
        }
    } else {
        Args::Create {
            center: coordinates(&first)?,
        }
    };
    match args.next() {
        Some(extra) => Err(format!("unexpected {extra}")),
        None => Ok(read),
    }
}

fn coordinates(text: &str) -> Result<(f64, f64), String> {
    let invalid = || format!("expected latitude,longitude in degrees, got {text}");
    let (latitude, longitude) = text.split_once(',').ok_or_else(invalid)?;
    let latitude: f64 = latitude.trim().parse().map_err(|_| invalid())?;
    let longitude: f64 = longitude.trim().parse().map_err(|_| invalid())?;
    Ok((latitude, longitude))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Result<Args, String> {
        parse_args(list.iter().map(|text| text.to_string()))
    }

    #[test]
    fn a_new_area_starts_where_it_is_told() {
        assert!(matches!(
            args(&["40.4,-3.7"]),
            Ok(Args::Create {
                center: (40.4, -3.7)
            })
        ));
    }

    #[test]
    fn editing_brings_the_area_and_its_points() {
        let read = args(&[
            "--edit",
            "Bödele ski resort",
            "47.4031,9.8132",
            "47.4178,9.8182",
        ]);

        let Ok(Args::Edit { area, summit, base }) = read else {
            panic!("not an edit");
        };
        assert_eq!(area, "Bödele ski resort");
        assert_eq!(summit, (47.4031, 9.8132));
        assert_eq!(base, Some((47.4178, 9.8182)));
        assert!(matches!(
            args(&["--edit", "Lachen", "47.41,9.35"]),
            Ok(Args::Edit { base: None, .. })
        ));
    }

    #[test]
    fn rejects_missing_or_bad_points() {
        assert!(args(&[]).is_err());
        assert!(args(&["north"]).is_err());
        assert!(args(&["--edit", "Lachen"]).is_err());
        assert!(args(&["40.4,-3.7", "41.0,-3.0"]).is_err());
    }
}
