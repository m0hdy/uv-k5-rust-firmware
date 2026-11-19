#![cfg_attr(not(test), no_std)]

//! Callable welcome startup and version-row transfer, independent of boot wiring.

use core::num::NonZeroU32;

use crate::st7565_display::{CommandStep, DisplayBlitPlan};
use crate::st7565_line_transport::{self, A0, LineError, LineTransport};
use crate::st7565_mmio_transport::{self, St7565MmioTransport};
use crate::st7565_startup_executor::target_spi0::{self, DelayMs, RegisterIo};
use crate::startup_version_display::{
    LCD_WIDTH, StartupDisplayMode, StartupVersionError, startup_version_display_plan,
};
use crate::startup_version_row_plan::{StartupVersionRowError, compose_startup_version_row};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WelcomeError {
    Version(StartupVersionError),
    Row(StartupVersionRowError),
    Initialization(LineError),
    Transfer(LineError),
}

struct Registers<'a, I>(&'a mut I);

impl<I: RegisterIo> st7565_mmio_transport::RegisterIo for Registers<'_, I> {
    fn read(&mut self, register: st7565_mmio_transport::RegisterAddress) -> u32 {
        self.0.read(register.address())
    }

    fn write(&mut self, register: st7565_mmio_transport::RegisterAddress, value: u32) {
        self.0.write(register.address(), value);
    }
}

fn transfer_row<I: RegisterIo>(
    io: &mut I,
    plan: DisplayBlitPlan<'_>,
    fifo_poll_limit: NonZeroU32,
) -> Result<(), LineError> {
    let mut registers = Registers(io);
    let mut transport = St7565MmioTransport { io: &mut registers };
    transport.set_spi_mode(false);
    let result = (|| {
        transport.set_a0(A0::Command);
        for step in plan.steps() {
            match step {
                CommandStep::Command(byte) => {
                    st7565_line_transport::write(&mut transport, byte, fifo_poll_limit)?;
                }
                CommandStep::Data(bytes) => {
                    st7565_line_transport::drain(&mut transport)?;
                    transport.set_a0(A0::Data);
                    for &byte in bytes {
                        st7565_line_transport::write(&mut transport, byte, fifo_poll_limit)?;
                    }
                    st7565_line_transport::drain(&mut transport)?;
                }
            }
        }
        Ok(())
    })();
    transport.set_spi_mode(true);
    result
}

/// Initializes the display and transfers the version row, using the package version.
pub fn welcome_with_io<I: RegisterIo, D: DelayMs + ?Sized>(
    io: &mut I,
    delay: &mut D,
    mode: StartupDisplayMode,
    pages: &mut [[u8; LCD_WIDTH]],
    fifo_poll_limit: NonZeroU32,
) -> Result<(), WelcomeError> {
    welcome_with_version_and_io(
        io,
        delay,
        mode,
        Some(env!("CARGO_PKG_VERSION")),
        pages,
        fifo_poll_limit,
    )
}

/// As [`welcome_with_io`], with an explicit version for focused callers and tests.
///
/// Rendering and geometry validation precede register access; rendering may
/// modify page 6 even if subsequent initialization or transfer fails.
pub fn welcome_with_version_and_io<I: RegisterIo, D: DelayMs + ?Sized>(
    io: &mut I,
    delay: &mut D,
    mode: StartupDisplayMode,
    version: Option<&str>,
    pages: &mut [[u8; LCD_WIDTH]],
    fifo_poll_limit: NonZeroU32,
) -> Result<(), WelcomeError> {
    let version = startup_version_display_plan(mode, version).map_err(WelcomeError::Version)?;
    let row = compose_startup_version_row(&version, pages).map_err(WelcomeError::Row)?;
    target_spi0::startup_with_io(io, delay, fifo_poll_limit)
        .map_err(WelcomeError::Initialization)?;
    if let Some(row) = row {
        transfer_row(io, row, fifo_poll_limit).map_err(WelcomeError::Transfer)?;
    }
    Ok(())
}

/// Displays the package version on an already-initialized controller.
pub fn welcome_on_initialized_display_with_io<I: RegisterIo>(
    io: &mut I,
    mode: StartupDisplayMode,
    pages: &mut [[u8; LCD_WIDTH]],
    fifo_poll_limit: NonZeroU32,
) -> Result<(), WelcomeError> {
    welcome_on_initialized_display_with_version_and_io(
        io,
        mode,
        Some(env!("CARGO_PKG_VERSION")),
        pages,
        fifo_poll_limit,
    )
}

/// As [`welcome_on_initialized_display_with_io`], with an explicit version.
///
/// The controller must already be initialized. Validation and rendering precede
/// register access; rendering may modify page 6 even if the transfer fails.
pub fn welcome_on_initialized_display_with_version_and_io<I: RegisterIo>(
    io: &mut I,
    mode: StartupDisplayMode,
    version: Option<&str>,
    pages: &mut [[u8; LCD_WIDTH]],
    fifo_poll_limit: NonZeroU32,
) -> Result<(), WelcomeError> {
    let version = startup_version_display_plan(mode, version).map_err(WelcomeError::Version)?;
    let row = compose_startup_version_row(&version, pages).map_err(WelcomeError::Row)?;
    if let Some(row) = row {
        transfer_row(io, row, fifo_poll_limit).map_err(WelcomeError::Transfer)?;
    }
    Ok(())
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct VolatileRegisters;

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl RegisterIo for VolatileRegisters {
    fn read(&mut self, address: usize) -> u32 {
        // SAFETY: welcome_mmio requires the mapped, aligned DP32G030 registers.
        unsafe { core::ptr::read_volatile(address as *const u32) }
    }

    fn write(&mut self, address: usize, value: u32) {
        // SAFETY: welcome_mmio requires exclusive, serialized register access.
        unsafe { core::ptr::write_volatile(address as *mut u32, value) }
    }
}

/// Runs the same welcome composition using volatile DP32G030 accesses.
///
/// # Safety
///
/// The caller must satisfy all mapping, clock, pin, idle-bus, delay and
/// whole-register ownership requirements of
/// [`target_spi0::startup_mmio`], continuing that exclusive ownership through
/// the row transfer. PB11 must be the display reset output, not live SWD;
/// PB9 must be the A0 output and B7/B8/B10 muxed to display SPI0. Every writer
/// to the shared registers, including interrupts and unrelated-bit writers,
/// must be excluded throughout the call and its delays. The delay must provide
/// the requested milliseconds and must not panic or access those registers.
/// FIFO and drain polling are bounded; successful return is not hardware
/// validation or evidence of actual firmware boot integration.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn welcome_mmio(
    delay: &mut dyn DelayMs,
    mode: StartupDisplayMode,
    pages: &mut [[u8; LCD_WIDTH]],
    fifo_poll_limit: NonZeroU32,
) -> Result<(), WelcomeError> {
    welcome_with_io(&mut VolatileRegisters, delay, mode, pages, fifo_poll_limit)
}

/// Displays the package version using an already-initialized controller.
///
/// # Safety
///
/// Controller initialization must already have succeeded. The caller must meet
/// the mapping, enabled clock, configured pin mux/output, idle SPI0 and exclusive
/// whole-register ownership requirements of
/// [`st7565_mmio_transport::draw_line_mmio`]. Serialize against all writers,
/// including interrupts and unrelated-bit writers, throughout the call.
/// FIFO and drain polling are bounded. This does not establish boot integration
/// or hardware validation.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn welcome_on_initialized_display_mmio(
    mode: StartupDisplayMode,
    pages: &mut [[u8; LCD_WIDTH]],
    fifo_poll_limit: NonZeroU32,
) -> Result<(), WelcomeError> {
    welcome_on_initialized_display_with_io(&mut VolatileRegisters, mode, pages, fifo_poll_limit)
}
