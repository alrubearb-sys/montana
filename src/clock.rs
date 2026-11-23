use crate::model::UnixTime;

const DAY: i64 = 86_400;

pub fn weekday(time: UnixTime, offset: i32) -> &'static str {
    // 1 January 1970 was a Thursday.
    match day(time, offset).rem_euclid(7) {
        0 => "Thu",
        1 => "Fri",
        2 => "Sat",
        3 => "Sun",
        4 => "Mon",
        5 => "Tue",
        _ => "Wed",
    }
}

/// Days since 1 January 1970 in local time.
pub fn day(time: UnixTime, offset: i32) -> i64 {
    (time.0 + i64::from(offset)).div_euclid(DAY)
}

pub fn hour(time: UnixTime, offset: i32) -> i64 {
    (time.0 + i64::from(offset)).rem_euclid(DAY) / 3600
}

pub fn clock(time: UnixTime, offset: i32) -> String {
    let seconds = (time.0 + i64::from(offset)).rem_euclid(DAY);
    format!("{:02}:{:02}", seconds / 3600, seconds % 3600 / 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_local_time() {
        let midnight = UnixTime(1795302000);

        assert_eq!(weekday(midnight, 3600), "Sun");
        assert_eq!(hour(midnight, 3600), 0);
        assert_eq!(clock(midnight, 3600), "00:00");
        assert_eq!(weekday(midnight, 0), "Sat");
        assert_eq!(clock(midnight, 0), "23:00");
    }
}
