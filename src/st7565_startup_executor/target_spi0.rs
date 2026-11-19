//! Callable SPI0/reset startup boundary; not connected to board boot.

#[allow(dead_code, unused_attributes)]
#[path = "../spi0_startup_adapter.rs"]
mod spi0_startup_adapter;

use super::{StartupTransport, execute_startup};
use crate::st7565_display::COLUMN_OFFSET;
use crate::st7565_line_transport::{self, A0, LineError, LineTransport};
use crate::st7565_mmio_transport::{self as mmio, St7565MmioTransport};
use crate::st7565_startup::ResetLevel;
use core::num::NonZeroU32;

const RESET_MASK: u32 = 1 << 11;

/// Aligned 32-bit register accesses; implementations must be infallible and not panic.
///
/// Addresses are supplied by the existing SPI0 and ST7565 register maps.
pub trait RegisterIo {
    fn read(&mut self, address: usize) -> u32;
    fn write(&mut self, address: usize, value: u32);
}

/// Caller-supplied millisecond delay; implementations must be infallible and not panic.
pub trait DelayMs {
    fn delay_ms(&mut self, milliseconds: u16);
}

struct Registers<'a, I>(&'a mut I);

impl<I: RegisterIo> mmio::RegisterIo for Registers<'_, I> {
    fn read(&mut self, register: mmio::RegisterAddress) -> u32 {
        self.0.read(register.address())
    }

    fn write(&mut self, register: mmio::RegisterAddress, value: u32) {
        self.0.write(register.address(), value);
    }
}

impl<I: RegisterIo> spi0_startup_adapter::RegisterIo for Registers<'_, I> {
    fn read(&mut self, register: spi0_startup_adapter::RegisterAddress) -> u32 {
        self.0.read(register.address())
    }

    fn write(&mut self, register: spi0_startup_adapter::RegisterAddress, value: u32) {
        self.0.write(register.address(), value);
    }
}

struct StartupBackend<'a, I, D: ?Sized> {
    registers: Registers<'a, I>,
    delay: &'a mut D,
    fill: Option<u8>,
}

impl<'a, I: RegisterIo, D: ?Sized> StartupBackend<'a, I, D> {
    fn line(&mut self) -> St7565MmioTransport<'_, Registers<'a, I>> {
        St7565MmioTransport {
            io: &mut self.registers,
        }
    }
}

impl<I: RegisterIo, D: ?Sized> LineTransport for StartupBackend<'_, I, D> {
    fn set_spi_mode(&mut self, enabled: bool) {
        self.line().set_spi_mode(enabled);
    }

    fn set_a0(&mut self, a0: A0) {
        self.line().set_a0(a0);
    }

    fn fifo_ready(&mut self) -> bool {
        self.line().fifo_ready()
    }

    fn write_byte(&mut self, byte: u8) {
        self.line().write_byte(byte);
    }

    fn drain_busy(&mut self) -> bool {
        self.line().drain_busy()
    }
}

impl<I: RegisterIo, D: DelayMs + ?Sized> StartupTransport for StartupBackend<'_, I, D> {
    fn initialize_spi0(&mut self) {
        spi0_startup_adapter::apply_with_io(&mut self.registers);
    }

    fn set_reset(&mut self, level: ResetLevel) {
        let address = mmio::RegisterAddress::GpioBData.address();
        let data = self.registers.0.read(address);
        let updated = match level {
            ResetLevel::High => data | RESET_MASK,
            ResetLevel::Low => data & !RESET_MASK,
        };
        self.registers.0.write(address, updated);
    }

    fn delay_ms(&mut self, milliseconds: u16) {
        self.delay.delay_ms(milliseconds);
    }

    fn fill_screen_handoff(&mut self, fill: u8) {
        // Defer fallible I/O until after the infallible executor hook returns.
        self.fill = Some(fill);
    }
}

fn select_zero_fill_pages(
    transport: &mut impl LineTransport,
    fifo_poll_limit: NonZeroU32,
) -> Result<(), LineError> {
    transport.set_spi_mode(false);
    let result = (|| {
        for page in 0..8 {
            transport.set_a0(A0::Command);
            for byte in [
                0xb0 + page,
                0x10 | (COLUMN_OFFSET >> 4),
                COLUMN_OFFSET & 0x0f,
            ] {
                st7565_line_transport::write(transport, byte, fifo_poll_limit)?;
            }
            st7565_line_transport::drain(transport)?;
            transport.set_a0(A0::Data);
            // Pinned DrawLine(..., NULL, 0) has no pixel writes, but still drains.
            st7565_line_transport::drain(transport)?;
        }
        Ok(())
    })();
    transport.set_spi_mode(true);
    result
}

/// Initializes SPI0, executes reset/startup, then completes the zero-fill handoff.
///
/// Every FIFO write uses the explicit nonzero per-byte poll budget. All drains
/// use the existing bounded `DRAIN_POLL_LIMIT`. Errors restore MSR_SSN and stop
/// subsequent commands/pages; no pixel bytes or boot-complete state are emitted.
pub fn startup_with_io<I: RegisterIo, D: DelayMs + ?Sized>(
    io: &mut I,
    delay: &mut D,
    fifo_poll_limit: NonZeroU32,
) -> Result<(), LineError> {
    let mut backend = StartupBackend {
        registers: Registers(io),
        delay,
        fill: None,
    };
    execute_startup(&mut backend, fifo_poll_limit)?;
    match backend.fill {
        Some(0) => select_zero_fill_pages(&mut backend, fifo_poll_limit),
        _ => Err(LineError::InvalidGeometry),
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct VolatileRegisters;

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl RegisterIo for VolatileRegisters {
    fn read(&mut self, address: usize) -> u32 {
        // SAFETY: startup_mmio requires the mapped, aligned DP32G030 registers.
        unsafe { core::ptr::read_volatile(address as *const u32) }
    }

    fn write(&mut self, address: usize, value: u32) {
        // SAFETY: startup_mmio requires exclusive, serialized register access.
        unsafe { core::ptr::write_volatile(address as *mut u32, value) }
    }
}

/// Runs the startup boundary with aligned volatile 32-bit target accesses.
///
/// # Safety
///
/// The caller must guarantee the DP32G030 mappings used by the SPI0 initializer
/// and ST7565 transport: SPI0 CR/WDR/IE/IF/FIFOST at 0x400b8000/04/10/14/18,
/// SYSCON device clock gate at 0x40000008, and GPIOB DATA at 0x40060800.
/// GPIOB and SPI0 clocks must already be enabled (CR is accessed before the
/// initializer's clock-gate update), status registers must be operational, and
/// the bus must be idle. B7/B8/B10 must be muxed for display SPI0. PB11 must be
/// a GPIO output owned as display reset, not SWD; PB9 must be an A0 GPIO output.
///
/// Throughout this entire call, including delays and the final handoff, the
/// caller must serialize against every other writer (including interrupts) to
/// SPI0 CR/WDR/IE/IF/FIFOST, GPIOB DATA, and SYSCON device clock gate, and retain
/// exclusive display/pin ownership. The delay must provide the requested
/// milliseconds without panicking or accessing these shared registers.
/// This function configures no GPIO direction/mux or arbitration and supplies
/// no delay calibration, physical timing guarantee, or board-boot wiring.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn startup_mmio(
    delay: &mut dyn DelayMs,
    fifo_poll_limit: NonZeroU32,
) -> Result<(), LineError> {
    startup_with_io(&mut VolatileRegisters, delay, fifo_poll_limit)
}
