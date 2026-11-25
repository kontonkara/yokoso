//! The clock: the time and the date as the config writes them, and when
//! what it says next changes, so it wakes only then.

use std::{fmt::Write, time::Duration};

use chrono::{DateTime, Local, TimeZone, Timelike};

use crate::config::Clock;

/// Whether `format` is one strftime takes: a broken one is the config's
/// mistake, said when it is read, never a blank clock.
pub fn check(format: &str) -> Result<(), String> {
    use chrono::format::{Item, StrftimeItems};
    if StrftimeItems::new(format).any(|item| matches!(item, Item::Error)) {
        return Err(format!("not a time format strftime takes: {format:?}"));
    }
    Ok(())
}

/// What it says at `now`: the time, and the date when it shows one.
pub fn says<Tz: TimeZone>(now: &DateTime<Tz>, config: &Clock) -> (String, Option<String>)
where
    Tz::Offset: std::fmt::Display,
{
    let written = |format: &str| {
        let mut out = String::new();
        let _ = write!(out, "{}", now.format(format));
        out
    };
    let date = config.date.then(|| written(&config.date_format));
    (written(&config.format), date)
}

/// Whether `format` shows seconds, and so changes every second.
fn ticks_seconds(format: &str) -> bool {
    let mut chars = format.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            continue;
        }
        // Flags and widths sit between the % and its letter.
        let letter = chars
            .by_ref()
            .find(|c| !matches!(c, '-' | '_' | '0' | '^' | '#' | ':' | '.' | '1'..='9'));
        if matches!(letter, Some('S' | 'T' | 's' | 'r' | 'X' | '+' | 'f')) {
            return true;
        }
    }
    false
}

/// How long from `now` until what it says can next change: the next whole
/// second where it shows seconds, else the next whole minute.
pub fn next_change<Tz: TimeZone>(now: &DateTime<Tz>, config: &Clock) -> Duration {
    let into_second = Duration::from_nanos(u64::from(now.nanosecond() % 1_000_000_000));
    let second = Duration::from_secs(1) - into_second;
    if ticks_seconds(&config.format) || (config.date && ticks_seconds(&config.date_format)) {
        return second;
    }
    second + Duration::from_secs(u64::from(59 - now.second().min(59)))
}

/// Now, where the clock is.
pub fn now() -> DateTime<Local> {
    Local::now()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, TimeZone};

    fn at(h: u32, m: u32, s: u32, ms: u32) -> DateTime<FixedOffset> {
        FixedOffset::east_opt(3600)
            .unwrap()
            .with_ymd_and_hms(2026, 11, 25, h, m, s)
            .unwrap()
            + chrono::Duration::milliseconds(i64::from(ms))
    }

    #[test]
    fn it_says_the_time_and_the_date_as_asked() {
        let mut config = Clock::default();
        assert_eq!(says(&at(9, 5, 0, 0), &config), ("09:05".into(), None));
        config.date = true;
        assert_eq!(says(&at(9, 5, 0, 0), &config).1.as_deref(), Some("Wed 25 Nov"));
    }

    #[test]
    fn it_wakes_when_what_it_says_changes_and_not_before() {
        let mut config = Clock::default();
        // Minutes: at the next whole minute.
        assert_eq!(next_change(&at(12, 0, 59, 500), &config), Duration::from_millis(500));
        assert_eq!(next_change(&at(12, 0, 0, 0), &config), Duration::from_secs(60));
        // Seconds, by any of their spellings: at the next whole second.
        for format in ["%H:%M:%S", "%T", "%-I:%M:%S %p", "%X"] {
            config.format = format.into();
            assert_eq!(
                next_change(&at(12, 0, 10, 250), &config),
                Duration::from_millis(750),
                "{format}"
            );
        }
        config.format = "%H:%M %%S".into();
        assert_eq!(next_change(&at(12, 0, 10, 250), &config), Duration::from_millis(49_750));
        assert!(ticks_seconds("%S") && !ticks_seconds("%H:%M"));
    }

    #[test]
    fn a_broken_format_is_found_before_it_shows() {
        assert!(check("%H:%M").is_ok());
        assert!(check("%H:%Q").is_err());
    }
}
