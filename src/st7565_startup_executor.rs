#![cfg_attr(not(test), no_std)]

//! Bounded execution of the pinned ST7565 startup plan.

#[path = "st7565_startup_executor/target_spi0.rs"]
pub mod target_spi0;

use core::num::NonZeroU32;

use crate::st7565_line_transport::{self, A0, LineError, LineTransport};
use crate::st7565_startup::{ResetLevel, SpiEffect, StartupStep, startup_steps};

/// Typed hooks around the existing reset, delay, SPI and display transport operations.
///
/// Implementations must be infallible and must not panic so transport timeouts
/// can restore SPI mode before returning.
pub trait StartupTransport: LineTransport {
    fn initialize_spi0(&mut self);
    fn set_reset(&mut self, level: ResetLevel);
    fn delay_ms(&mut self, milliseconds: u16);
    fn fill_screen_handoff(&mut self, fill: u8);
}

/// Executes the pinned startup plan through bounded FIFO and drain polling.
///
/// The final fill-screen step is passed to the caller only after restoring SPI
/// mode. Its implementation owns the page-selection-only `FillScreen(0)`
/// behavior; this executor does not write pixel bytes.
pub fn execute_startup<T: StartupTransport>(
    transport: &mut T,
    fifo_poll_limit: NonZeroU32,
) -> Result<(), LineError> {
    use SpiEffect::{DrainTransmitFifo, InitializeSpi0, SetMasterSelect, WriteCommand};
    use StartupStep::{DelayMs, FillScreen, Reset, Spi};

    let mut master_select_disabled = false;
    let result = (|| {
        for step in startup_steps() {
            match step {
                Spi(InitializeSpi0) => transport.initialize_spi0(),
                Reset(level) => transport.set_reset(level),
                DelayMs(milliseconds) => transport.delay_ms(milliseconds),
                Spi(SetMasterSelect(enabled)) => {
                    transport.set_spi_mode(enabled);
                    master_select_disabled = !enabled;
                }
                Spi(WriteCommand(byte)) => {
                    transport.set_a0(A0::Command);
                    st7565_line_transport::write(transport, byte, fifo_poll_limit)?;
                }
                Spi(DrainTransmitFifo) => st7565_line_transport::drain(transport)?,
                FillScreen(fill) => {
                    if master_select_disabled {
                        transport.set_spi_mode(true);
                        master_select_disabled = false;
                    }
                    transport.fill_screen_handoff(fill);
                }
            }
        }
        Ok(())
    })();

    if result.is_err() && master_select_disabled {
        transport.set_spi_mode(true);
    }

    result
}
