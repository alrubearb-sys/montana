const DAY: i64 = 86_400;

pub fn weekday(time: i64, offset: i64) -> &'static str {
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

pub fn day(time: i64, offset: i64) -> i64 {
    (time + offset).div_euclid(DAY)
}

pub fn hour(time: i64, offset: i64) -> i64 {
    (time + offset).rem_euclid(DAY) / 3600
}

pub fn clock(time: i64, offset: i64) -> String {
    let seconds = (time + offset).rem_euclid(DAY);
    format!("{:02}:{:02}", seconds / 3600, seconds % 3600 / 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_local_time() {
        assert_eq!(weekday(1795302000, 3600), "Sun");
        assert_eq!(hour(1795302000, 3600), 0);
        assert_eq!(clock(1795302000, 0), "23:00");
        assert_eq!(weekday(1795302000, 0), "Sat");
    }
}
