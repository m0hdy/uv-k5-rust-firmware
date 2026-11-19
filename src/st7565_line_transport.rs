#![cfg_attr(not(test), no_std)]

//! Bounded, allocation-free execution of a checked ST7565 line plan.

#[allow(dead_code, unused_attributes)]
#[path = "st7565_display.rs"]
mod st7565_display;

use core::num::NonZeroU32;
use st7565_display::{CommandStep, DisplayBlitPlan};

/// Maximum number of drain-status reads, matching the pinned driver.
pub const DRAIN_POLL_LIMIT: u32 = 100_001;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum A0 {
    Command,
    Data,
}

/// Operations required by the line executor; status methods perform one read.
///
/// Implementations must make these operations infallible and must not panic:
/// otherwise the executor cannot guarantee mode restoration.
pub trait LineTransport {
    fn set_spi_mode(&mut self, enabled: bool);
    fn set_a0(&mut self, a0: A0);
    fn fifo_ready(&mut self) -> bool;
    fn write_byte(&mut self, byte: u8);
    fn drain_busy(&mut self) -> bool;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineError {
    InvalidGeometry,
    FifoTimeout,
    DrainTimeout,
}

fn write<T: LineTransport>(
    transport: &mut T,
    byte: u8,
    fifo_poll_limit: NonZeroU32,
) -> Result<(), LineError> {
    for _ in 0..fifo_poll_limit.get() {
        if transport.fifo_ready() {
            transport.write_byte(byte);
            return Ok(());
        }
    }
    Err(LineError::FifoTimeout)
}

fn drain<T: LineTransport>(transport: &mut T) -> Result<(), LineError> {
    for _ in 0..DRAIN_POLL_LIMIT {
        if !transport.drain_busy() {
            return Ok(());
        }
    }
    Err(LineError::DrainTimeout)
}

/// Draws a nonempty line, rejecting invalid geometry before any transport I/O.
///
/// `fifo_poll_limit` bounds status reads separately for each byte. Once SPI
/// mode has been switched, it is restored even if either polling limit expires.
pub fn draw_line<T: LineTransport>(
    transport: &mut T,
    page: u16,
    column: u16,
    payload: &[u8],
    fifo_poll_limit: NonZeroU32,
) -> Result<(), LineError> {
    let plan =
        DisplayBlitPlan::draw_line(page, column, payload).ok_or(LineError::InvalidGeometry)?;
    transport.set_spi_mode(false);
    let result = (|| {
        transport.set_a0(A0::Command);
        for step in plan.steps() {
            match step {
                CommandStep::Command(byte) => write(transport, byte, fifo_poll_limit)?,
                CommandStep::Data(bytes) => {
                    drain(transport)?;
                    transport.set_a0(A0::Data);
                    for &byte in bytes {
                        write(transport, byte, fifo_poll_limit)?;
                    }
                    drain(transport)?;
                }
            }
        }
        Ok(())
    })();
    transport.set_spi_mode(true);
    result
}
