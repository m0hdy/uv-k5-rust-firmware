#![cfg_attr(not(test), no_std)]

//! Sequences board startup, fallible startup settings, and bounded battery sampling.

#[allow(dead_code, unused_attributes, unused_imports)]
#[path = "board_startup_adapters.rs"]
mod board_startup_adapters;
#[allow(dead_code, unused_attributes)]
#[path = "startup_battery_sampling.rs"]
mod startup_battery_sampling;

use crate::i2c_bitbang_flow::target_gpio::Delay1us;
use crate::settings_boot_composition;
pub use crate::settings_boot_composition::{
    Acknowledgement, I2cTransport, ReadError, ReadPlan, SettingsBoot, SettingsBootBlock,
    SettingsBootReadError,
};
use crate::st7565_startup_executor::target_spi0::DelayMs;
pub use board_startup_adapters::{
    Call as BoardCall, Defines as BoardDefines, StartupError as BoardStartupError, StartupIo,
};
use core::num::NonZeroU32;
use startup_battery_sampling::sample_startup_battery;
pub use startup_battery_sampling::{
    ReadRegister as BatteryReadRegister, RegisterIo as BatteryRegisterIo, StartupBatteryError,
    StartupBatteryReadings, WriteRegister as BatteryWriteRegister,
};

pub trait BootIo: StartupIo + I2cTransport + BatteryRegisterIo {}

impl<T> BootIo for T where T: StartupIo + I2cTransport + BatteryRegisterIo {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootConfig {
    pub board: BoardDefines,
    pub enable_swd: bool,
    /// Caller-selected SARADC EXTTRIG_SEL; the board initializer provides no value.
    pub exttrig_sel: u16,
    /// Caller-selected nonzero per-byte ST7565 FIFO polling budget.
    pub st7565_fifo_poll_limit: NonZeroU32,
    pub settings: settings_boot_composition::Features,
    pub battery_poll_budget: usize,
    pub battery_calibration: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApplicationStartup {
    pub settings: SettingsBoot,
    pub battery: StartupBatteryReadings,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootError<E, F> {
    Board(BoardStartupError<F>),
    Settings(SettingsBootReadError<E>),
    Battery(StartupBatteryError),
}

/// Returns application startup state only after every required stage succeeds.
pub fn coordinate<IO: BootIo>(
    io: &mut IO,
    display_delay: &mut dyn DelayMs,
    fm_delay: &mut dyn Delay1us,
    config: BootConfig,
) -> Result<ApplicationStartup, BootError<<IO as I2cTransport>::Error, <IO as StartupIo>::FmError>>
{
    board_startup_adapters::run(
        config.board,
        config.enable_swd,
        config.exttrig_sel,
        display_delay,
        fm_delay,
        config.st7565_fifo_poll_limit,
        io,
    )
    .map_err(BootError::Board)?;
    let settings = settings_boot_composition::compose_with_transport(io, config.settings)
        .map_err(BootError::Settings)?;
    let battery =
        sample_startup_battery(io, config.battery_poll_budget, config.battery_calibration)
            .map_err(BootError::Battery)?;

    Ok(ApplicationStartup { settings, battery })
}
