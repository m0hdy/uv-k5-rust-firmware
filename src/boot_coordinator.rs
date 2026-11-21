#![cfg_attr(not(test), no_std)]

//! Sequences board startup, fallible startup settings, and bounded battery sampling.

#[allow(dead_code, unused_attributes, unused_imports)]
#[path = "board_startup_adapters.rs"]
mod board_startup_adapters;
use crate::i2c_bitbang_flow::target_gpio::Delay1us;
use crate::settings_boot_composition;
pub use crate::settings_boot_composition::{
    Acknowledgement, I2cTransport, ReadError, ReadPlan, SettingsBoot, SettingsBootBlock,
    SettingsBootReadError, SettingsBootWithWelcome,
};
use crate::st7565_startup_executor::target_spi0::DelayMs;
use crate::st7565_startup_executor::target_spi0::RegisterIo as DisplayRegisterIo;
use crate::startup_version_display::LCD_WIDTH;
use crate::welcome_startup::{self, WelcomeError};
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub struct TargetBoardHardwareIo(board_startup_adapters::HardwareIo);

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl TargetBoardHardwareIo {
    /// Creates the concrete DP32G030 board startup backend.
    ///
    /// # Safety
    ///
    /// The caller must execute on the supported DP32G030/Cortex-M0 mapping
    /// with the required privilege and operational source clock. It must own
    /// all touched clock, PORTCON, GPIO, PWM, SARADC, SPI0, CRC, and optional
    /// FM registers for the entire boot sequence, serializing every
    /// read-modify-write against unrelated-bit writers and interrupt handlers.
    /// The active vector table must contain the valid SysTick return handler;
    /// SARADC interrupt delivery must remain disabled for this bounded startup.
    pub unsafe fn new() -> Self {
        Self(board_startup_adapters::HardwareIo)
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl StartupIo for TargetBoardHardwareIo {
    type FmError = core::convert::Infallible;

    fn device_clock_gates(&mut self) {
        self.0.device_clock_gates();
    }

    fn systick(&mut self, period_ticks: u32) -> Result<(), BoardStartupError<Self::FmError>> {
        self.0.systick(period_ticks)
    }

    fn portcon(&mut self, enable_swd: bool) {
        self.0.portcon(enable_swd);
    }

    fn gpio(&mut self, enable_fmradio: bool) {
        self.0.gpio(enable_fmradio);
    }

    fn backlight_init_hardware(&mut self) {
        self.0.backlight_init_hardware();
    }

    fn adc_init(&mut self, exttrig_sel: u16) {
        self.0.adc_init(exttrig_sel);
    }

    fn st7565_init(
        &mut self,
        delay: &mut dyn DelayMs,
        fifo_poll_limit: NonZeroU32,
    ) -> Result<(), crate::st7565_line_transport::LineError> {
        self.0.st7565_init(delay, fifo_poll_limit)
    }

    fn bk1080_shutdown(
        &mut self,
        delay: &mut dyn Delay1us,
    ) -> Result<(), crate::bk1080_startup_adapter::ShutdownError<Self::FmError>> {
        self.0.bk1080_shutdown(delay)
    }

    fn crc_init(&mut self) {
        self.0.crc_init();
    }
}
use crate::startup_battery_sampling::sample_startup_battery;
pub use crate::startup_battery_sampling::{
    ReadRegister as BatteryReadRegister, RegisterIo as BatteryRegisterIo, StartupBatteryError,
    StartupBatteryReadings, WriteRegister as BatteryWriteRegister,
};
pub use board_startup_adapters::{
    Call as BoardCall, Defines as BoardDefines, StartupError as BoardStartupError, StartupIo,
};
use core::num::NonZeroU32;

pub trait BootIo: StartupIo + I2cTransport + BatteryRegisterIo {
    fn i2c_handoff(&mut self);
}

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
pub struct ApplicationStartupWithWelcome {
    pub settings: SettingsBoot,
    pub battery: StartupBatteryReadings,
    pub welcome_mode: crate::startup_version_display::StartupDisplayMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootError<E, F> {
    Board(BoardStartupError<F>),
    Settings(SettingsBootReadError<E>),
    Battery(StartupBatteryError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FirmwareStartupError<E, F> {
    Boot(BootError<E, F>),
    Welcome(WelcomeError),
}

/// Returns application startup state only after every required stage succeeds.
pub fn coordinate<IO: BootIo>(
    io: &mut IO,
    display_delay: &mut dyn DelayMs,
    fm_delay: &mut dyn Delay1us,
    config: BootConfig,
) -> Result<ApplicationStartup, BootError<<IO as I2cTransport>::Error, <IO as StartupIo>::FmError>>
{
    run_board(io, display_delay, fm_delay, config)?;
    let settings = match settings_boot_composition::compose_with_transport(io, config.settings) {
        Ok(settings) => settings,
        Err(error) => {
            io.i2c_handoff();
            return Err(BootError::Settings(error));
        }
    };
    io.i2c_handoff();
    let battery = sample_battery(io, config).map_err(BootError::Battery)?;

    Ok(ApplicationStartup { settings, battery })
}

/// Runs the concrete board/settings/battery startup and continues on its initialized display.
#[inline(never)]
pub fn coordinate_with_welcome<IO: BootIo + DisplayRegisterIo>(
    io: &mut IO,
    display_delay: &mut dyn DelayMs,
    fm_delay: &mut dyn Delay1us,
    config: BootConfig,
    version: Option<&str>,
    pages: &mut [[u8; LCD_WIDTH]],
) -> Result<
    ApplicationStartupWithWelcome,
    FirmwareStartupError<<IO as I2cTransport>::Error, <IO as StartupIo>::FmError>,
> {
    run_board(io, display_delay, fm_delay, config).map_err(FirmwareStartupError::Boot)?;
    let settings =
        match settings_boot_composition::compose_with_welcome_transport(io, config.settings) {
            Ok(settings) => settings,
            Err(error) => {
                io.i2c_handoff();
                return Err(FirmwareStartupError::Boot(BootError::Settings(error)));
            }
        };
    io.i2c_handoff();
    let battery = sample_battery(io, config)
        .map_err(|error| FirmwareStartupError::Boot(BootError::Battery(error)))?;
    let welcome_mode = settings.welcome_mode;
    welcome_startup::welcome_on_initialized_display_with_version_and_io(
        io,
        welcome_mode,
        version,
        pages,
        config.st7565_fifo_poll_limit,
    )
    .map_err(FirmwareStartupError::Welcome)?;
    Ok(ApplicationStartupWithWelcome {
        settings: settings.settings,
        battery,
        welcome_mode,
    })
}

fn run_board<IO: BootIo>(
    io: &mut IO,
    display_delay: &mut dyn DelayMs,
    fm_delay: &mut dyn Delay1us,
    config: BootConfig,
) -> Result<(), BootError<<IO as I2cTransport>::Error, <IO as StartupIo>::FmError>> {
    board_startup_adapters::run(
        config.board,
        config.enable_swd,
        config.exttrig_sel,
        display_delay,
        fm_delay,
        config.st7565_fifo_poll_limit,
        io,
    )
    .map_err(BootError::Board)
}

fn sample_battery<IO: BatteryRegisterIo>(
    io: &mut IO,
    config: BootConfig,
) -> Result<StartupBatteryReadings, StartupBatteryError> {
    sample_startup_battery(io, config.battery_poll_budget, config.battery_calibration)
}
