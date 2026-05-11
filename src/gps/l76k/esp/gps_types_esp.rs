use esp_hal::gpio::{Level, Output, OutputConfig, OutputPin};

use crate::gps::GpsOutput;

/// ESP32 pin bundle required by the L76K hardware driver.
pub struct GpsHw<REINIT, STANDBY>
where
    REINIT: OutputPin + 'static,
    STANDBY: OutputPin + 'static,
{
    pub reinit: REINIT,
    pub standby: STANDBY,
}

/// Thin `GpsOutput` wrapper over `esp_hal::gpio::Output`.
pub struct EspOutput<'d>(pub Output<'d>);

impl<'d> GpsOutput for EspOutput<'d> {
    /// Sets GPIO high.
    fn set_high(&mut self) {
        self.0.set_high();
    }

    /// Sets GPIO low.
    fn set_low(&mut self) {
        self.0.set_low();
    }
}

impl<REINIT, STANDBY> GpsHw<REINIT, STANDBY>
where
    REINIT: OutputPin + 'static,
    STANDBY: OutputPin + 'static,
{
    /// Converts raw pins into configured output drivers.
    pub fn into_outputs(self) -> (EspOutput<'static>, EspOutput<'static>) {
        (
            EspOutput(Output::new(
                self.reinit,
                Level::High,
                OutputConfig::default(),
            )),
            EspOutput(Output::new(
                self.standby,
                Level::High,
                OutputConfig::default(),
            )),
        )
    }
}
