//! L76K-specific driver, command encoder, and platform adapters.

#[cfg(any(feature = "nrf", feature = "_esp", test))]
mod driver;
pub mod pcas;

#[cfg(feature = "nrf")]
pub mod nrf;

#[cfg(feature = "_esp")]
pub mod esp;
