#![cfg_attr(not(test), no_std)]

//! Target-only register adapter for the bounded ST7565 line transport.

#[allow(dead_code, unused_attributes)]
#[path = "st7565_line_transport.rs"]
mod st7565_line_transport;

use core::num::NonZeroU32;
pub use st7565_line_transport::LineError;
use st7565_line_transport::{A0, LineTransport};

pub const SPI_CR_MSR_SSN_MASK: u32 = 1 << 12;
pub const SPI_FIFOST_TFF_MASK: u32 = 1 << 4;
pub const SPI_IF_TX_BUSY_MASK: u32 = 0x20;
pub const GPIOB_ST7565_A0_MASK: u32 = 1 << 9;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegisterAddress {
    SpiControl,
    SpiWriteData,
    SpiInterruptFlag,
    SpiFifoStatus,
    GpioBData,
}

impl RegisterAddress {
    pub const fn address(self) -> usize {
        match self {
            Self::SpiControl => 0x400b_8000,
            Self::SpiWriteData => 0x400b_8004,
            Self::SpiInterruptFlag => 0x400b_8014,
            Self::SpiFifoStatus => 0x400b_8018,
            Self::GpioBData => 0x4006_0800,
        }
    }
}

/// Register access boundary; implementations must be infallible and must not panic.
pub trait RegisterIo {
    fn read(&mut self, register: RegisterAddress) -> u32;
    fn write(&mut self, register: RegisterAddress, value: u32);
}

struct St7565MmioTransport<'a, I> {
    io: &'a mut I,
}

impl<I: RegisterIo> LineTransport for St7565MmioTransport<'_, I> {
    fn set_spi_mode(&mut self, enabled: bool) {
        let control = self.io.read(RegisterAddress::SpiControl);
        let updated = if enabled {
            control | SPI_CR_MSR_SSN_MASK
        } else {
            control & !SPI_CR_MSR_SSN_MASK
        };
        self.io.write(RegisterAddress::SpiControl, updated);
    }

    fn set_a0(&mut self, a0: A0) {
        let data = self.io.read(RegisterAddress::GpioBData);
        let updated = match a0 {
            A0::Command => data & !GPIOB_ST7565_A0_MASK,
            A0::Data => data | GPIOB_ST7565_A0_MASK,
        };
        self.io.write(RegisterAddress::GpioBData, updated);
    }

    fn fifo_ready(&mut self) -> bool {
        self.io.read(RegisterAddress::SpiFifoStatus) & SPI_FIFOST_TFF_MASK == 0
    }

    fn write_byte(&mut self, byte: u8) {
        self.io
            .write(RegisterAddress::SpiWriteData, u32::from(byte));
    }

    fn drain_busy(&mut self) -> bool {
        self.io.read(RegisterAddress::SpiInterruptFlag) & SPI_IF_TX_BUSY_MASK != 0
    }
}

pub fn draw_line_with_io<I: RegisterIo>(
    io: &mut I,
    page: u16,
    column: u16,
    payload: &[u8],
    fifo_poll_limit: NonZeroU32,
) -> Result<(), LineError> {
    st7565_line_transport::draw_line(
        &mut St7565MmioTransport { io },
        page,
        column,
        payload,
        fifo_poll_limit,
    )
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct VolatileRegisterIo;

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl RegisterIo for VolatileRegisterIo {
    fn read(&mut self, register: RegisterAddress) -> u32 {
        let address = register.address() as *const u32;
        // SAFETY: The caller of draw_line_mmio guarantees the DP32G030 mapping.
        unsafe { core::ptr::read_volatile(address) }
    }

    fn write(&mut self, register: RegisterAddress, value: u32) {
        let address = register.address() as *mut u32;
        // SAFETY: The caller of draw_line_mmio guarantees the DP32G030 mapping.
        unsafe { core::ptr::write_volatile(address, value) }
    }
}

/// Executes one checked line transfer using volatile 32-bit DP32G030 accesses.
///
/// # Safety
///
/// The caller must ensure the DP32G030 register mappings match
/// [`RegisterAddress::address`], GPIOB bit 9 is muxed as the ST7565 A0 GPIO,
/// and SPI0 clock and GPIOB clock are enabled. SPI0 must already be configured
/// for the display with transmit FIFO status and interrupt flags operational.
/// The SPI bus must be idle before entry, and the caller must have exclusive
/// ownership of SPI0 CR, WDR, IF, FIFOST, and GPIOB DATA for the whole call.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn draw_line_mmio(
    page: u16,
    column: u16,
    payload: &[u8],
    fifo_poll_limit: NonZeroU32,
) -> Result<(), LineError> {
    draw_line_with_io(
        &mut VolatileRegisterIo,
        page,
        column,
        payload,
        fifo_poll_limit,
    )
}
