//! L76K-specific driver, command encoder, and platform adapters.

#[cfg(any(feature = "nrf", feature = "esp", test))]
mod driver;
pub mod pcas;

#[cfg(feature = "nrf")]
pub mod nrf;

#[cfg(feature = "esp")]
pub mod esp;

#[allow(unused)]
#[macro_export]
macro_rules! log_error {
    ( $($arg:tt)+ ) => {
        #[cfg(feature = "defmt")]
        defmt::error!($($arg)+);
        #[cfg(feature = "log-04")]
        log::error!($($arg)+);
    };
}

#[allow(unused)]
#[macro_export]
macro_rules! log_warn {
    ( $($arg:tt)+ ) => {
        #[cfg(feature = "defmt")]
        defmt::warn!($($arg)+);
        #[cfg(feature = "log-04")]
        log::warn!($($arg)+);
    };
}

#[allow(unused)]
#[macro_export]
macro_rules! log_info {
    ( $($arg:tt)+ ) => {
        #[cfg(feature = "defmt")]
        defmt::info!($($arg)+);
        #[cfg(feature = "log-04")]
        log::info!($($arg)+);
    };
}

#[allow(unused)]
#[macro_export]
macro_rules! log_debug {
    ( $($arg:tt)+ ) => {
        #[cfg(feature = "defmt")]
        defmt::debug!($($arg)+);
        #[cfg(feature = "log-04")]
        log::debug!($($arg)+);
    };
}
