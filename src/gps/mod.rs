//! Module layout for GPS support.
//! - core traits (`GpsDriver`, `GpsFsm`, `GpsOutput`).
//! - `l76k`: concrete implementation for the Quectel L76K module.
//! - `l76k::pcas`: encoder for PCAS configuration commands.
//! - `l76k::{nrf,esp}`: platform glue for pin/output setup and FSM wiring.

pub mod gps_interfases;
pub mod l76k;

pub use gps_interfases::{GpsDriver, GpsFsm, GpsOutput};
