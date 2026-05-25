use embassy_time::{Duration, Timer};
use embedded_io_async::{ErrorType, Read, Write};

use crate::gps::l76k::driver::L76kGps;
use crate::gps::l76k::esp::{EspOutput, GpsHw};
use crate::gps::l76k::pcas::EncodedCommand;
use crate::gps::{GpsDriver, GpsFsm, GpsOutput};
use crate::types::{GpsError, GpsEvent, GpsState};

/// L76K finite-state machine specialized for ESP32 platforms.
pub struct L76kFsm<IO, RESET, STANDBY>
where
    IO: Read + Write + ErrorType,
    RESET: GpsOutput,
    STANDBY: GpsOutput,
{
    gps: L76kGps<IO, RESET, STANDBY>,
    state: GpsState,
}

impl<IO> L76kFsm<IO, EspOutput<'static>, EspOutput<'static>>
where
    IO: Read + Write + ErrorType,
{
    /// Creates an ESP32-C3 FSM from hardware pins and UART factory.
    pub async fn new_sep<REINIT, STANDBY, F>(hw: GpsHw<REINIT, STANDBY>, make_io: F) -> Self
    where
        REINIT: esp_hal::gpio::OutputPin + 'static,
        STANDBY: esp_hal::gpio::OutputPin + 'static,
        F: FnOnce() -> IO,
    {
        let (reinit, standby) = hw.into_outputs();

        Self {
            gps: L76kGps::new(reinit, standby, make_io).await,
            state: GpsState::SetupConnection,
        }
    }

    /// Sends a raw PCAS command through the underlying driver.
    pub async fn execute_command<const N: usize>(
        &mut self,
        cmd: &EncodedCommand<N>,
        timeout: Duration,
    ) -> Result<(), GpsError<IO::Error>> {
        self.gps.execute_command(cmd, timeout).await
    }
}

impl<IO: Read + Write + ErrorType> GpsFsm for L76kFsm<IO, EspOutput<'static>, EspOutput<'static>> {
    type Event = GpsEvent;
    type Error = GpsError<IO::Error>;

    /// Advances FSM state and emits an event when available.
    async fn step(&mut self) -> Result<Option<Self::Event>, Self::Error> {
        match self.state {
            GpsState::Reset => {
                self.gps.reset().await;
                self.state = GpsState::SetupConnection;
                Ok(None)
            }
            GpsState::SetupConnection => {
                self.gps.prepare().await;
                self.gps.configure_default_output().await?;
                self.state = GpsState::Running;
                Ok(None)
            }
            GpsState::Running => match self.gps.run().await {
                Ok(event) => Ok(Some(event)),
                Err(err) => {
                    crate::log_warn!("gps uart error");
                    self.state = GpsState::Recover;
                    Err(err)
                }
            },
            GpsState::Recover => {
                Timer::after(Duration::from_millis(400)).await;
                self.state = GpsState::Reset;
                Ok(None)
            }
        }
    }
}
