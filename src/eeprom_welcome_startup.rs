//! Acquire the EEPROM welcome mode, then execute the concrete welcome startup.

use core::num::NonZeroU32;

use crate::eeprom_read_transport::{I2cTransport, ReadError, execute_read, plan_read};
use crate::st7565_startup_executor::target_spi0::{DelayMs, RegisterIo};
use crate::startup_version_display::{LCD_WIDTH, StartupDisplayMode};
use crate::welcome_startup::{self, WelcomeError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EepromWelcomeError<E> {
    Acquisition(ReadError<E>),
    Welcome(WelcomeError),
}

fn acquire_mode<T: I2cTransport>(
    transport: &mut T,
) -> Result<StartupDisplayMode, ReadError<T::Error>> {
    let mut data = [0; 8];
    execute_read(transport, plan_read(0x0e90, 8), &mut data)?;
    // Pinned settings.c load policy, independent of Rust discriminants/write codecs.
    Ok(match data[7] {
        0 => StartupDisplayMode::FullScreen,
        1 => StartupDisplayMode::Custom,
        2 => StartupDisplayMode::Voltage,
        3 => StartupDisplayMode::None,
        _ => StartupDisplayMode::Voltage,
    })
}

/// Reads the mode and executes welcome startup with the package version.
///
/// Acquisition must finish, including STOP, before any display access, delay or
/// page modification. Transport failures are never replaced with a default.
pub fn welcome_with_io<T: I2cTransport, I: RegisterIo, D: DelayMs + ?Sized>(
    transport: &mut T,
    io: &mut I,
    delay: &mut D,
    pages: &mut [[u8; LCD_WIDTH]],
    fifo_poll_limit: NonZeroU32,
) -> Result<(), EepromWelcomeError<T::Error>> {
    welcome_with_version_and_io(
        transport,
        io,
        delay,
        Some(env!("CARGO_PKG_VERSION")),
        pages,
        fifo_poll_limit,
    )
}

/// As [`welcome_with_io`], with an explicit project version.
///
/// After acquisition, the rendering, page mutation and error semantics are
/// exactly those of [`welcome_startup::welcome_with_version_and_io`].
pub fn welcome_with_version_and_io<T: I2cTransport, I: RegisterIo, D: DelayMs + ?Sized>(
    transport: &mut T,
    io: &mut I,
    delay: &mut D,
    version: Option<&str>,
    pages: &mut [[u8; LCD_WIDTH]],
    fifo_poll_limit: NonZeroU32,
) -> Result<(), EepromWelcomeError<T::Error>> {
    let mode = acquire_mode(transport).map_err(EepromWelcomeError::Acquisition)?;
    welcome_startup::welcome_with_version_and_io(io, delay, mode, version, pages, fifo_poll_limit)
        .map_err(EepromWelcomeError::Welcome)
}

/// Uses the existing GPIO-bitbang EEPROM backend and volatile welcome entry.
///
/// The bus is stopped and handed back to the keypad before display startup,
/// including on acquisition failure. Both delay providers are caller-supplied;
/// this entry supplies no default timing or board setup.
///
/// # Safety
///
/// The caller must satisfy all requirements of
/// [`crate::i2c_bitbang_flow::target_gpio::GpioI2cIo::new_target`] and
/// [`welcome_startup::welcome_mmio`]: mapped/enabled registers, board-initialized
/// PA10/PA11, exclusive GPIO/keypad ownership through handoff, configured display
/// mux/outputs, idle SPI0, and PB11 reset ownership rather than live SWD.
/// Serialize against every writer of the shared whole registers, including
/// interrupts and unrelated-bit writers, across operations, delays and handoff.
/// Delays must be accurate, non-panicking and must not access shared registers.
/// Polling remains bounded; this is not boot wiring or hardware validation.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn welcome_mmio<D: crate::i2c_bitbang_flow::target_gpio::Delay1us>(
    i2c_delay: D,
    display_delay: &mut dyn DelayMs,
    pages: &mut [[u8; LCD_WIDTH]],
    fifo_poll_limit: NonZeroU32,
) -> Result<(), EepromWelcomeError<core::convert::Infallible>> {
    use crate::eeprom_i2c_bridge::I2cBitbangTransport;
    use crate::i2c_bitbang_flow::target_gpio::GpioI2cIo;

    // SAFETY: the caller guarantees GPIO mapping, board setup and serialization.
    let mut gpio = unsafe { GpioI2cIo::new_target(i2c_delay) };
    let mode = acquire_mode(&mut I2cBitbangTransport::new(&mut gpio));
    gpio.handoff_to_keypad();
    let mode = mode.map_err(EepromWelcomeError::Acquisition)?;
    // SAFETY: the caller retains exclusive configured display ownership.
    unsafe { welcome_startup::welcome_mmio(display_delay, mode, pages, fifo_poll_limit) }
        .map_err(EepromWelcomeError::Welcome)
}
