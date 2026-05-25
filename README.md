# embassy_gps

`embassy_gps` is a lightweight wrapper for using GPS modules with Embassy-based projects.

## What it provides

- Async GPS abstractions (`GpsDriver`, `GpsFsm`, and shared event/error types).
- A working implementation for the **L76K** module.
- Platform glue for current targets (`nrf` and `esp32c3` features).
- PCAS command encoding for runtime GPS configuration.

## Architecture

The crate is split into generic traits and module-specific implementations, so adding new GPS chips should stay straightforward:

- keep common traits/events in shared modules,
- add a chip-specific driver module,
- add platform adapters only where needed.

Right now, L76K is implemented; additional drivers can be added with the same pattern.

## Feature Flags

Feature flags are used to differentiate between targets and to select a logging library:

- `defmt` (default): Log using `defmt`
- `log-04`: Log using `log` v0.4
- `nrf`: Target nRF processor
- `esp32-c3`: Target ESP32-C3 processor

## Small example (nRF)

Pseudo-code (board/UART init omitted) showing the normal flow:

```rust
use embassy_gps::gps::GpsFsm;
use embassy_gps::gps::l76k::nrf::{GpsHw, L76kFsm};
use embassy_gps::gps::l76k::pcas::{PcasCommand, PcasGnssMode};
use embassy_gps::types::GpsEvent;
use embassy_time::Duration;

async fn gps_task(hw: GpsHw<impl embassy_nrf::gpio::Pin, impl embassy_nrf::gpio::Pin>) {
    let mut gps = L76kFsm::new_nrf(hw, || {
        // Return your UART transport implementing Read + Write + ErrorType.
        make_uart()
    })
    .await;

    // Optional: send a PCAS command after startup.
    let cmd = PcasCommand::Pcas04SetGnss(PcasGnssMode::GpsBeiDou)
        .encode::<64>()
        .unwrap();
    let _ = gps.execute_command(&cmd, Duration::from_secs(1)).await;

    loop {
        match gps.step().await {
            Ok(Some(GpsEvent::Fix(fix))) => {
                // Use fix.lat_microdeg / fix.lon_microdeg / fix.sats / fix.utc_time_ms
            }
            Ok(Some(GpsEvent::Sentence(_))) => {}
            Ok(Some(GpsEvent::BytesDetected)) | Ok(None) => {}
            Err(_) => {
                // FSM will recover automatically.
            }
        }
    }
}
```

## Small example (ESP32-C3)

Same flow, but with ESP HAL types:

```rust
use embassy_gps::gps::GpsFsm;
use embassy_gps::gps::l76k::esp32c3::{GpsHw, L76kFsm};
use embassy_gps::types::GpsEvent;

async fn gps_task(hw: GpsHw<impl esp_hal::gpio::OutputPin, impl esp_hal::gpio::OutputPin>) {
    let mut gps = L76kFsm::new_sep(hw, || {
        // Return your UART transport implementing Read + Write + ErrorType.
        make_uart()
    })
    .await;

    loop {
        match gps.step().await {
            Ok(Some(GpsEvent::Fix(fix))) => {
                // Use fix.lat_microdeg / fix.lon_microdeg / fix.sats / fix.utc_time_ms
            }
            Ok(Some(_)) | Ok(None) => {}
            Err(_) => {
                // FSM will recover automatically.
            }
        }
    }
}
```

## Hardware status

Tested on **T-Echo GPS** hardware and confirmed working.

## License

This project is licensed under **AGPL** (GNU Affero General Public License).
