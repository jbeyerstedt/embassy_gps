use core::option::Option;

use chrono::Timelike;
use nmea::{Nmea, SentenceType};

/// Compact GPS fix data used by higher-level consumers.
///
/// Note: Since we only return this when GGA or RMC data was received, we can assume that some fields are present.
#[derive(Clone, Copy, Debug)]
pub struct GpsFix {
    pub time: chrono::NaiveTime,
    pub date: Option<chrono::NaiveDate>,
    pub latitude_deg: f64,
    pub longitude_deg: f64,
    pub altitude_m: Option<f32>,
    pub speed_over_ground: Option<f32>,
    pub true_course_deg: Option<f32>,
    pub sats: u8,
    pub hdop: Option<f32>,
}

impl GpsFix {
    /// Builds a fix from the currently parsed NMEA state.
    #[must_use]
    pub fn from(nmea: &Nmea) -> Option<Self> {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "source (GGA message) only support 0-12 satellites"
        )]
        let sats = nmea.fix_satellites().unwrap_or(0) as u8;

        Some(GpsFix {
            time: nmea.fix_time?,
            date: nmea.fix_date,
            latitude_deg: nmea.latitude?,
            longitude_deg: nmea.longitude?,
            altitude_m: nmea.altitude,
            speed_over_ground: nmea.speed_over_ground,
            true_course_deg: nmea.true_course,
            sats,
            hdop: nmea.hdop,
        })
    }

    /// Combines date and time information to full timestamp, if available
    #[must_use]
    pub fn get_timestamp(&self) -> Option<chrono::NaiveDateTime> {
        self.date.as_ref().map(|date| date.and_time(self.time))
    }

    /// Combines date and time information to full timestamp, if available
    #[must_use]
    pub fn get_timestamp_millis(&self) -> Option<i64> {
        self.get_timestamp().map(|t| t.and_utc().timestamp_millis())
    }

    /// Returns UTC milliseconds since midnight (when `time` is available)
    #[must_use]
    pub fn get_utc_time_millis(&self) -> u64 {
        u64::from(self.time.num_seconds_from_midnight()) * 1_000
            + u64::from(self.time.nanosecond() / 1_000_000)
    }

    /// Returns the latitude in micro-degrees
    #[must_use]
    pub fn lat_microdeg(&self) -> i32 {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "value range is +-90_000_000 which fits in i32"
        )]
        let microdeg = (self.latitude_deg * 1_000_000.0) as i32;
        microdeg
    }
    /// Returns the longitude in micro-degrees
    #[must_use]
    pub fn lon_microdeg(&self) -> i32 {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "value range is +-90_000_000 which fits in i32"
        )]
        let microdeg = (self.longitude_deg * 1_000_000.0) as i32;
        microdeg
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
