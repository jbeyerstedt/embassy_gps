use core::option::Option;

use chrono::Timelike;
use nmea::{Nmea, SentenceType};

/// Compact GPS fix data used by higher-level consumers.
#[derive(Clone, Copy, Debug)]
pub struct GpsFix {
    pub lat_microdeg: i32,
    pub lon_microdeg: i32,
    pub sats: u8,
    pub time: Option<chrono::NaiveTime>,
    pub date: Option<chrono::NaiveDate>,
}

impl GpsFix {
    /// Builds a fix from the currently parsed NMEA state.
    #[must_use]
    pub fn from(nmea: &Nmea) -> Option<Self> {
        let time = nmea.fix_time;
        let date = nmea.fix_date;

        #[allow(
            clippy::cast_possible_truncation,
            reason = "source (GGA message) only support 0-12 satellites"
        )]
        let sats = nmea.fix_satellites().unwrap_or(0) as u8;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "value range is +-90_000_000 which fits in i32"
        )]
        let lat_microdeg = (nmea.latitude()? * 1_000_000.0) as i32;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "value range is +-180_000_000 which fits in i32"
        )]
        let lon_microdeg = (nmea.longitude()? * 1_000_000.0) as i32;

        Some(GpsFix {
            lat_microdeg,
            lon_microdeg,
            sats,
            time,
            date,
        })
    }

    /// Combines date and time information to full timestamp, if available
    #[must_use]
    pub fn get_timestamp(&self) -> Option<chrono::NaiveDateTime> {
        if let (Some(date), Some(time)) = (&self.date, &self.time) {
            Some(date.and_time(*time))
        } else {
            None
        }
    }

    /// Combines date and time information to full timestamp, if available
    #[must_use]
    pub fn get_timestamp_millis(&self) -> Option<i64> {
        self.get_timestamp().map(|t| t.and_utc().timestamp_millis())
    }

    /// Returns UTC milliseconds since midnight (when `time` is available)
    #[must_use]
    pub fn get_utc_time_millis(&self) -> Option<u64> {
        self.time.map(|t| {
            u64::from(t.num_seconds_from_midnight()) * 1_000 + u64::from(t.nanosecond() / 1_000_000)
        })
    }
}

/// Events emitted by the driver/FSM pipeline.
#[derive(Clone, Copy, Debug)]
pub enum GpsEvent {
    BytesDetected,
    Sentence(SentenceType),
    Fix(GpsFix),
}

/// Internal states used by the recovery-oriented FSM.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GpsState {
    Reset,
    SetupConnection,
    Running,
    Recover,
}

/// Unified error type for GPS IO, parsing, and setup.
#[derive(Debug)]
pub enum GpsError<IO> {
    IO(IO),
    Utf8,
    NotReady,
    BaudNotDetected,
    Timeout,
    BuildCommand,
}
