#![cfg_attr(not(test), no_std)]

//! Offline composition of startup battery acquisition and voltage calibration.

#[allow(unused_attributes)]
#[path = "adc_battery_acquisition.rs"]
mod adc_battery_acquisition;
use adc_battery_acquisition::acquire_battery_samples;
pub use adc_battery_acquisition::adc_samples::CalibrationError;
use adc_battery_acquisition::adc_samples::calibrated_battery_voltage;
pub use adc_battery_acquisition::{AcquisitionError, ReadRegister, RegisterIo, WriteRegister};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupBatteryReadings {
    pub voltage_samples: [u16; 4],
    pub current: u16,
    pub calibrated_voltage: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupBatteryError {
    Calibration(CalibrationError),
    Acquisition {
        sample_index: usize,
        error: AcquisitionError,
    },
}

/// Acquires four samples in order, then calibrates their average.
///
/// Zero calibration is rejected before any I/O. Acquisition errors identify
/// the zero-based sample index; earlier I/O is not rolled back.
pub fn sample_startup_battery(
    io: &mut impl RegisterIo,
    poll_budget: usize,
    calibration: u16,
) -> Result<StartupBatteryReadings, StartupBatteryError> {
    if calibration == 0 {
        return Err(StartupBatteryError::Calibration(
            CalibrationError::ZeroCalibration,
        ));
    }

    let mut voltage_samples = [0; 4];
    let mut current = 0;
    for (sample_index, voltage) in voltage_samples.iter_mut().enumerate() {
        let samples = acquire_battery_samples(io, poll_budget).map_err(|error| {
            StartupBatteryError::Acquisition {
                sample_index,
                error,
            }
        })?;
        *voltage = samples.voltage;
        current = samples.current;
    }

    let calibrated_voltage = calibrated_battery_voltage(voltage_samples, calibration)
        .map_err(StartupBatteryError::Calibration)?;
    Ok(StartupBatteryReadings {
        voltage_samples,
        current,
        calibrated_voltage,
    })
}
