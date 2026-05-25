use embassy_futures::{select::Either, select::select};
use embassy_time::{Duration, Timer};
use embedded_io_async::{ErrorType, Read, Write};
use nmea::{Nmea, SentenceType};

use crate::{
    gps::{
        GpsDriver, GpsOutput,
        l76k::pcas::{EncodedCommand, Pcas03, PcasCommand, PcasSentenceRate},
    },
    types::{GpsError, GpsEvent, GpsFix},
};

/// Core L76K UART driver with line parsing and command TX.
#[cfg_attr(not(any(feature = "nrf", feature = "esp")), allow(dead_code))]
pub(super) struct L76kGps<IO, RESET, STANDBY>
where
    IO: Read + Write + ErrorType,
    RESET: GpsOutput,
    STANDBY: GpsOutput,
{
    io: Option<IO>,
    reinit: RESET,
    _standby: STANDBY,
    nmea: Nmea,
    line: [u8; 128],
    idx: usize,
    pending: heapless::Deque<GpsEvent, 8>,
}

#[cfg_attr(not(any(feature = "nrf", feature = "esp")), allow(dead_code))]
impl<IO, RESET, STANDBY> L76kGps<IO, RESET, STANDBY>
where
    IO: Read + Write + ErrorType,
    RESET: GpsOutput,
    STANDBY: GpsOutput,
{
    /// Creates the driver and performs an initial hardware reset.
    pub(super) async fn new<F>(reinit: RESET, _standby: STANDBY, make_io: F) -> Self
    where
        F: FnOnce() -> IO,
    {
        let mut tmp = Self {
            io: None,
            reinit,
            _standby,
            nmea: Nmea::default(),
            line: [0; 128],
            idx: 0,
            pending: heapless::Deque::new(),
        };

        tmp.reset().await;

        tmp.io = Some(make_io());
        tmp
    }

    /// Feeds one UART byte into line buffering and NMEA parsing.
    fn consume_byte(&mut self, byte: u8) -> Result<Option<GpsEvent>, GpsError<IO::Error>> {
        match byte {
            b'\r' => Ok(None),
            b'\n' => {
                let Some(line_len) = self.finish_line()? else {
                    return Ok(None);
                };

                let sentence =
                    core::str::from_utf8(&self.line[..line_len]).map_err(|_| GpsError::Utf8)?;

                let parsed = match self.nmea.parse(sentence) {
                    Ok(kind) => kind,
                    Err(_) => return Ok(None),
                };

                if matches!(parsed, SentenceType::GGA | SentenceType::RMC)
                    && let Some(fix) = GpsFix::from(&self.nmea)
                {
                    return Ok(Some(GpsEvent::Fix(fix)));
                }

                Ok(Some(GpsEvent::Sentence(parsed)))
            }
            byte => {
                if let Some(slot) = self.line.get_mut(self.idx) {
                    *slot = byte;
                    self.idx += 1;
                } else {
                    self.idx = 0;
                    defmt::warn!("gps line overflow");
                }
                Ok(None)
            }
        }
    }

    /// Finalizes the current buffered NMEA line.
    fn finish_line(&mut self) -> Result<Option<usize>, GpsError<IO::Error>> {
        if self.idx == 0 {
            return Ok(None);
        }

        let len = self.idx;
        self.idx = 0;
        Ok(Some(len))
    }

    /// Applies the default sentence filter for normal runtime.
    pub async fn configure_default_output(&mut self) -> Result<(), GpsError<IO::Error>> {
        let cfg = Pcas03 {
            gga: PcasSentenceRate::every_n(1),
            gll: PcasSentenceRate::OFF,
            gsa: PcasSentenceRate::OFF,
            gsv: PcasSentenceRate::OFF,
            rmc: PcasSentenceRate::every_n(1),
            vtg: PcasSentenceRate::OFF,
            zda: PcasSentenceRate::OFF,
            ant: PcasSentenceRate::OFF,
        };

        let cmd = PcasCommand::Pcas03SetSentenceRates(cfg)
            .encode::<64>()
            .map_err(|_| GpsError::BuildCommand)?;

        self.execute_command(&cmd, Duration::from_secs(1)).await
    }
}

impl<IO, RESET, STANDBY> GpsDriver for L76kGps<IO, RESET, STANDBY>
where
    IO: Read + Write + ErrorType,
    RESET: GpsOutput,
    STANDBY: GpsOutput,
{
    type Event = GpsEvent;
    type Error = GpsError<IO::Error>;

    /// Pulses REINIT and waits until the module is ready again.
    async fn reset(&mut self) {
        defmt::debug!("gps reset pulse start");
        self.reinit.set_low();
        Timer::after(Duration::from_millis(150)).await;
        self.reinit.set_high();
        defmt::debug!("gps reset pulse done");
        Timer::after(Duration::from_secs(2)).await;
    }

    /// Clears parser state before entering active mode.
    async fn prepare(&mut self) {
        self.idx = 0;
        self.nmea = Nmea::default();
    }

    /// Reads UART bytes until a GPS event can be emitted.
    async fn run(&mut self) -> Result<Self::Event, Self::Error> {
        let mut buf = [0u8; 64];

        loop {
            if let Some(event) = self.pending.pop_front() {
                return Ok(event);
            }

            let read_result = {
                let io = self.io.as_mut().ok_or(GpsError::NotReady)?;
                select(io.read(&mut buf), Timer::after(Duration::from_secs(10))).await
            };

            match read_result {
                Either::First(Ok(0)) => continue,
                Either::First(Ok(n)) => {
                    for &byte in &buf[..n] {
                        if let Some(event) = self.consume_byte(byte)? {
                            if self.pending.push_back(event).is_err() {
                                defmt::warn!("gps pending queue overflow");
                            }
                        }
                    }

                    if let Some(event) = self.pending.pop_front() {
                        return Ok(event);
                    }
                }
                Either::First(Err(e)) => return Err(GpsError::IO(e)),
                Either::Second(_) => return Err(GpsError::Timeout),
            }
        }
    }

    /// Writes an encoded PCAS command with timeout protection.
    async fn execute_command<const N: usize>(
        &mut self,
        cmd: &EncodedCommand<N>,
        timeout: Duration,
    ) -> Result<(), GpsError<IO::Error>> {
        let io = self.io.as_mut().ok_or(GpsError::NotReady)?;
        let bytes = cmd.as_bytes();
        let mut written = 0;

        while written < bytes.len() {
            let write_result = select(io.write(&bytes[written..]), Timer::after(timeout)).await;

            match write_result {
                Either::First(Ok(0)) => {
                    return Err(GpsError::Timeout);
                }
                Either::First(Ok(n)) => {
                    written += n;
                }
                Either::First(Err(e)) => {
                    return Err(GpsError::IO(e));
                }
                Either::Second(_) => {
                    return Err(GpsError::Timeout);
                }
            }
        }

        let flush_result = select(io.flush(), Timer::after(timeout)).await;
        match flush_result {
            Either::First(Ok(())) => Ok(()),
            Either::First(Err(e)) => Err(GpsError::IO(e)),
            Either::Second(_) => Err(GpsError::Timeout),
        }
    }
}

#[cfg(test)]
mod tests {
    use embassy_futures::block_on;
    use embedded_io_async::{ErrorType, Read, Write};
    use heapless::Vec;

    use super::*;

    /// Small no_std UART mock used by unit tests.
    struct MockUart<const RX: usize, const TX: usize> {
        rx: [u8; RX],
        rx_len: usize,
        rx_pos: usize,
        tx: Vec<u8, TX>,
    }

    impl<const RX: usize, const TX: usize> MockUart<RX, TX> {
        fn with_rx(data: &[u8]) -> Self {
            assert!(data.len() <= RX);
            let mut rx = [0u8; RX];
            rx[..data.len()].copy_from_slice(data);

            Self {
                rx,
                rx_len: data.len(),
                rx_pos: 0,
                tx: Vec::new(),
            }
        }

        fn written(&self) -> &[u8] {
            self.tx.as_slice()
        }
    }

    impl<const RX: usize, const TX: usize> ErrorType for MockUart<RX, TX> {
        type Error = embedded_io_async::ErrorKind;
    }

    impl<const RX: usize, const TX: usize> Read for MockUart<RX, TX> {
        async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            let available = self.rx_len.saturating_sub(self.rx_pos);
            if available == 0 {
                return Ok(0);
            }

            let n = core::cmp::min(available, buf.len());
            buf[..n].copy_from_slice(&self.rx[self.rx_pos..self.rx_pos + n]);
            self.rx_pos += n;
            Ok(n)
        }
    }

    impl<const RX: usize, const TX: usize> Write for MockUart<RX, TX> {
        async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
            let free = TX.saturating_sub(self.tx.len());
            let n = core::cmp::min(free, buf.len());
            if n == 0 && !buf.is_empty() {
                return Ok(0);
            }

            self.tx
                .extend_from_slice(&buf[..n])
                .map_err(|_| embedded_io_async::ErrorKind::OutOfMemory)?;
            Ok(n)
        }

        async fn flush(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct DummyPin;

    impl GpsOutput for DummyPin {
        fn set_high(&mut self) {}
        fn set_low(&mut self) {}
    }

    fn make_test_driver() -> L76kGps<MockUart<1, 256>, DummyPin, DummyPin> {
        L76kGps {
            io: Some(MockUart::with_rx(&[])),
            reinit: DummyPin,
            _standby: DummyPin,
            nmea: Nmea::default(),
            line: [0; 128],
            idx: 0,
            pending: heapless::Deque::new(),
        }
    }

    #[test]
    fn mock_uart_reads_seeded_data() {
        let mut uart = MockUart::<8, 8>::with_rx(b"ABC");
        let mut buf = [0u8; 8];

        let n = block_on(uart.read(&mut buf)).expect("read must succeed");
        assert_eq!(n, 3);
        assert_eq!(&buf[..n], b"ABC");

        let n = block_on(uart.read(&mut buf)).expect("second read must succeed");
        assert_eq!(n, 0);
    }

    #[test]
    fn mock_uart_captures_written_data() {
        let mut uart = MockUart::<1, 16>::with_rx(&[]);

        block_on(async {
            uart.write_all(b"PING")
                .await
                .expect("write_all must succeed");
            uart.flush().await.expect("flush must succeed");
        });

        assert_eq!(uart.written(), b"PING");
    }

    #[test]
    fn consume_gga_populates_utc_millis_in_fix() {
        let mut gps = make_test_driver();
        let sentence = b"$GPGGA,123519.000,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*59\r\n";

        let mut got_fix = None;
        for byte in sentence {
            if let Some(event) = gps.consume_byte(*byte).expect("line parse must succeed")
                && let GpsEvent::Fix(fix) = event
            {
                got_fix = Some(fix);
                break;
            }
        }

        let fix = got_fix.expect("must emit fix from GGA");
        assert_eq!(fix.utc_time_ms, Some(45_319_000));
    }
}
