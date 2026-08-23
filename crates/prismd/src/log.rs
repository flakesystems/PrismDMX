//! The structured logger `CLAUDE.md` asks for, and nothing more.
//!
//! Four levels, a target, a message, and a wall-clock timestamp on every line.
//! `CLAUDE.md` forbids plain `println!` in production code and the workspace
//! lints enforce it, so this is the one place in the daemon that writes to a
//! terminal.
//!
//! # Why it is written here rather than taken from a crate
//!
//! The same reason `prism-protocols` writes its own Art-Net packet: the seam a
//! logging framework would need is larger than the thing itself, and every line
//! of what is here is testable. [`format_line`] is a pure function over a
//! timestamp, a level, a target and a message, so "what does a log line look
//! like" is an assertion rather than a screenshot.
//!
//! # What it is deliberately not
//!
//! It is not the lock-free ring `ARCHITECTURE_SPEC.md` §3.1 requires for the
//! tick thread. **Nothing on the tick path logs**, which is why that ring does
//! not exist yet: the tick thread's body writes into atomics and the daemon
//! reads them. A later session that wants a log record out of the tick has to
//! build the ring first, and this module is where its writer would drain to.

use std::io::Write as _;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// How much to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Level {
    /// Everything, including what only matters while working on the daemon.
    Debug,
    /// The daemon's life: what it opened, what connected, what it wrote.
    #[default]
    Info,
    /// Something an operator should know about that did not stop the show.
    Warn,
    /// Something failed.
    Error,
    /// Nothing at all. Not a level a record can have — only one a filter can.
    Off,
}

/// The domain's spelling of a level, and this crate's, converted in **one**
/// place — S37.
///
/// Two types on purpose: `prism_domain::LogLevel` is what travels and what a
/// settings window draws, and this one is what a logger switches on. They would
/// be one type only if `prism-domain` were allowed to know what a logger is.
impl From<prism_domain::LogLevel> for Level {
    fn from(level: prism_domain::LogLevel) -> Self {
        match level {
            prism_domain::LogLevel::Debug => Self::Debug,
            prism_domain::LogLevel::Info => Self::Info,
            prism_domain::LogLevel::Warn => Self::Warn,
            prism_domain::LogLevel::Error => Self::Error,
            prism_domain::LogLevel::Off => Self::Off,
        }
    }
}

impl From<Level> for prism_domain::LogLevel {
    fn from(level: Level) -> Self {
        match level {
            Level::Debug => Self::Debug,
            Level::Info => Self::Info,
            Level::Warn => Self::Warn,
            Level::Error => Self::Error,
            Level::Off => Self::Off,
        }
    }
}

impl Level {
    /// The five names, as they are written on a command line and in a line of
    /// output.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
            Self::Off => "OFF",
        }
    }

    /// A level from what somebody typed, in any case.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text.to_ascii_lowercase().as_str() {
            "debug" => Some(Self::Debug),
            "info" => Some(Self::Info),
            "warn" | "warning" => Some(Self::Warn),
            "error" => Some(Self::Error),
            "off" | "none" => Some(Self::Off),
            _ => None,
        }
    }

    const fn as_u8(self) -> u8 {
        match self {
            Self::Debug => 0,
            Self::Info => 1,
            Self::Warn => 2,
            Self::Error => 3,
            Self::Off => 4,
        }
    }

    const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Debug,
            1 => Self::Info,
            2 => Self::Warn,
            3 => Self::Error,
            _ => Self::Off,
        }
    }
}

/// The filter, as a number so it can be read without a lock from any thread.
static FILTER: AtomicU8 = AtomicU8::new(1);

/// Sets the level below which records are discarded.
pub fn set_level(level: Level) {
    FILTER.store(level.as_u8(), Ordering::Relaxed);
}

/// The level currently in force.
#[must_use]
pub fn level() -> Level {
    Level::from_u8(FILTER.load(Ordering::Relaxed))
}

/// Whether a record at `level` would be written.
///
/// Public so a caller can skip building a message that would be thrown away.
#[must_use]
pub fn enabled(level: Level) -> bool {
    level != Level::Off && level.as_u8() >= FILTER.load(Ordering::Relaxed)
}

/// Writes one record, if the filter admits it.
///
/// Standard error rather than standard output: the daemon's output *is* DMX,
/// and a shell that pipes `prismd` somewhere should get diagnostics on the
/// stream diagnostics belong on.
pub fn record(level: Level, target: &str, message: &str) {
    if !enabled(level) {
        return;
    }
    let line = format_line(now(), level, target, message);
    // A logger that could fail the daemon would be worse than one that loses a
    // line: standard error is closed in a service with no console, and the show
    // is not affected by either.
    let _ = writeln!(std::io::stderr(), "{line}");
}

/// Something only useful while working on the daemon.
pub fn debug(target: &str, message: &str) {
    record(Level::Debug, target, message);
}

/// The daemon's life.
pub fn info(target: &str, message: &str) {
    record(Level::Info, target, message);
}

/// Something an operator should know about.
pub fn warn(target: &str, message: &str) {
    record(Level::Warn, target, message);
}

/// Something failed.
pub fn error(target: &str, message: &str) {
    record(Level::Error, target, message);
}

/// Seconds and milliseconds since the Unix epoch.
///
/// Saturating rather than panicking on a clock set before 1970: a desk with a
/// dead battery should still log.
fn now() -> (i64, u32) {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or_else(
        |before| {
            let duration = before.duration();
            (
                -i64::try_from(duration.as_secs()).unwrap_or(i64::MAX),
                duration.subsec_millis(),
            )
        },
        |since| {
            (
                i64::try_from(since.as_secs()).unwrap_or(i64::MAX),
                since.subsec_millis(),
            )
        },
    )
}

/// One line of output: `2026-08-12T18:04:11.123Z INFO  engine  message`.
///
/// The timestamp is UTC and in the format that sorts as text, because a log a
/// school hands to somebody a week later is read with `sort` and `grep`.
#[must_use]
pub fn format_line(now: (i64, u32), level: Level, target: &str, message: &str) -> String {
    let (seconds, millis) = now;
    format!(
        "{} {:<5} {target}: {message}",
        timestamp(seconds, millis),
        level.name()
    )
}

/// An ISO 8601 instant in UTC, to the millisecond.
fn timestamp(unix_seconds: i64, millis: u32) -> String {
    let days = unix_seconds.div_euclid(86_400);
    let seconds_of_day = unix_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// The civil date `days` days after 1970-01-01, by Howard Hinnant's algorithm.
///
/// Written out rather than taken from a date library for the reason in the
/// module documentation: it is twelve lines, it is exact for every day in the
/// proleptic Gregorian calendar, and it is testable against dates anybody can
/// check.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    // Shift the epoch to 0000-03-01, which puts the leap day at the end of the
    // year and makes every month length a straight-line function of the month.
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = u32::try_from(day_of_year - (153 * month_prime + 2) / 5 + 1).unwrap_or(1);
    let month = u32::try_from(if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    })
    .unwrap_or(1);
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::{Level, civil_from_days, enabled, format_line, level, set_level, timestamp};

    /// The domain's five levels and this crate's are the same five, converted in
    /// one place — S37.
    ///
    /// A round trip rather than a table: the pair that matters is the one a
    /// settings window writes and the logger switches on, and a conversion that
    /// dropped a level would show up here as a level that came back as another.
    #[test]
    fn the_domains_levels_and_this_crates_are_the_same_five() {
        for level in [
            Level::Debug,
            Level::Info,
            Level::Warn,
            Level::Error,
            Level::Off,
        ] {
            let travelled: prism_domain::LogLevel = level.into();
            assert_eq!(Level::from(travelled), level, "{level:?}");
        }
        assert_eq!(Level::from(prism_domain::LogLevel::default()), Level::Info);
    }

    #[test]
    fn a_level_survives_its_own_name() {
        for expected in [Level::Debug, Level::Info, Level::Warn, Level::Error] {
            assert_eq!(Level::parse(expected.name()), Some(expected));
            assert_eq!(
                Level::parse(&expected.name().to_lowercase()),
                Some(expected)
            );
        }
        assert_eq!(Level::parse("warning"), Some(Level::Warn));
        assert_eq!(Level::parse("none"), Some(Level::Off));
        assert_eq!(Level::parse("off"), Some(Level::Off));
        assert_eq!(Level::Off.name(), "OFF");
        assert_eq!(Level::parse("chatty"), None);
        assert_eq!(Level::default(), Level::Info);
    }

    #[test]
    fn a_filter_admits_its_own_level_and_everything_above_it() {
        set_level(Level::Warn);
        assert_eq!(level(), Level::Warn);
        assert!(!enabled(Level::Debug));
        assert!(!enabled(Level::Info));
        assert!(enabled(Level::Warn));
        assert!(enabled(Level::Error));

        set_level(Level::Off);
        for record in [Level::Debug, Level::Info, Level::Warn, Level::Error] {
            assert!(!enabled(record), "{record:?} got past an Off filter");
        }
        // Off is not a level a record can have, so it is never admitted either.
        assert!(!enabled(Level::Off));

        set_level(Level::Debug);
        assert!(enabled(Level::Debug));
        set_level(Level::Info);
    }

    /// The dates are ones anybody can check, and they cover the two cases the
    /// algorithm is actually about: a leap day, and the century that is not a
    /// leap year.
    #[test]
    fn a_civil_date_is_the_date_it_should_be() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        // 1900 is the century that is not a leap year, so the day after
        // 1900-02-28 is 1900-03-01 and the algorithm has to know it.
        assert_eq!(civil_from_days(-25_509), (1900, 2, 28));
        assert_eq!(civil_from_days(-25_508), (1900, 3, 1));
        assert_eq!(civil_from_days(20_677), (2026, 8, 12));
    }

    #[test]
    fn a_timestamp_is_utc_and_sorts_as_text() {
        // 2026-08-12T18:04:11.123Z
        assert_eq!(
            timestamp(1_786_557_851, 123),
            "2026-08-12T18:04:11.123Z".to_owned()
        );
        assert_eq!(timestamp(0, 0), "1970-01-01T00:00:00.000Z".to_owned());
        // A clock set before the epoch still produces a readable line rather
        // than a panic in the middle of a show.
        assert_eq!(timestamp(-1, 500), "1969-12-31T23:59:59.500Z".to_owned());
    }

    #[test]
    fn a_line_carries_the_time_the_level_the_target_and_the_message() {
        assert_eq!(
            format_line((1_786_557_851, 7), Level::Warn, "engine", "44 Hz"),
            "2026-08-12T18:04:11.007Z WARN  engine: 44 Hz"
        );
        assert_eq!(
            format_line((0, 0), Level::Info, "lock", "taken"),
            "1970-01-01T00:00:00.000Z INFO  lock: taken"
        );
    }

    #[test]
    fn writing_a_record_does_not_fail_whatever_the_filter_is() {
        set_level(Level::Debug);
        super::debug("test", "debug");
        super::info("test", "info");
        super::warn("test", "warn");
        super::error("test", "error");
        set_level(Level::Off);
        super::error("test", "not written");
        set_level(Level::Info);
    }
}
