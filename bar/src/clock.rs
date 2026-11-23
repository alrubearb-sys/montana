const DAY: i64 = 86_400;

/// "08:05" at a place `offset` seconds from UTC.
pub fn clock(time: i64, offset: i64) -> String {
    let seconds = (time + offset).rem_euclid(DAY);
    format!("{:02}:{:02}", seconds / 3600, seconds % 3600 / 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_local_time() {
        assert_eq!(clock(1795302000, 0), "23:00");
        assert_eq!(clock(1795302000, 3600), "00:00");
    }
}
