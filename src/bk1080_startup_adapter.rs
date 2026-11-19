//! Standalone BK1080 zero-frequency startup shutdown.

use crate::eeprom_read_transport::{Acknowledgement, I2cTransport};

const GPIOB_DATA: usize = 0x4006_0800;
const PB15: u32 = 1 << 15;

pub trait WordIo {
    fn read_word(&mut self, address: usize) -> u32;
    fn write_word(&mut self, address: usize, value: u32);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShutdownError<E> {
    NotAcknowledged,
    Transport(E),
}

fn shutdown_transaction<T: I2cTransport>(transport: &mut T) -> Result<(), ShutdownError<T::Error>> {
    transport.start().map_err(ShutdownError::Transport)?;
    for byte in [0x80, 0x04, 0x02, 0x41] {
        let result = match transport.write_byte(byte) {
            Ok(Acknowledgement::Ack) => Ok(()),
            Ok(Acknowledgement::Nack) => Err(ShutdownError::NotAcknowledged),
            Err(error) => Err(ShutdownError::Transport(error)),
        };
        if let Err(error) = result {
            let _ = transport.stop();
            return Err(error);
        }
    }
    transport.stop().map_err(ShutdownError::Transport)
}

/// Shuts down the receiver and asserts PB15 after transaction cleanup.
///
/// A failed START is not stopped. A write failure or NACK gets one cleanup
/// STOP, whose error is discarded; a failed final STOP is not retried. PB15
/// is asserted even on error, using a fresh read of GPIOB DATA. The infallible
/// word I/O and transport methods must not panic.
pub fn shutdown_with_io<T: I2cTransport, G: WordIo>(
    transport: &mut T,
    gpio: &mut G,
) -> Result<(), ShutdownError<T::Error>> {
    let result = shutdown_transaction(transport);
    let data = gpio.read_word(GPIOB_DATA);
    gpio.write_word(GPIOB_DATA, data | PB15);
    result
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct TargetWords;

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl WordIo for TargetWords {
    fn read_word(&mut self, address: usize) -> u32 {
        // SAFETY: the caller of shutdown_mmio guarantees the register mapping.
        unsafe { core::ptr::read_volatile(address as *const u32) }
    }

    fn write_word(&mut self, address: usize, value: u32) {
        // SAFETY: the caller of shutdown_mmio guarantees the register mapping.
        unsafe { core::ptr::write_volatile(address as *mut u32, value) }
    }
}

/// Performs shutdown using the existing GPIO bitbang owner and volatile GPIOB.
///
/// # Safety
///
/// The caller must meet all requirements of `GpioI2cIo::new_target`, including
/// mapped and enabled GPIOA/PORTCON registers, board-initialized PA10/PA11,
/// exclusive shared bus and keypad ownership, and serialization against every
/// whole-register writer through cleanup and handoff. GPIOB DATA must also be
/// mapped, enabled and accessible as an aligned word, with PB15 initialized as
/// an output. Serialize against all GPIOB DATA writers, including interrupts and
/// unrelated-bit writers, through the operation, cleanup and handoff. The
/// caller-supplied one-microsecond delay must be accurate, non-panicking and
/// must not access shared registers.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn shutdown_mmio<D: crate::i2c_bitbang_flow::target_gpio::Delay1us>(
    delay: D,
) -> Result<(), ShutdownError<core::convert::Infallible>> {
    use crate::eeprom_i2c_bridge::I2cBitbangTransport;
    use crate::i2c_bitbang_flow::target_gpio::GpioI2cIo;

    // SAFETY: the caller guarantees the mapping, setup and exclusive access.
    let mut pins = unsafe { GpioI2cIo::new_target(delay) };
    let result = shutdown_with_io(&mut I2cBitbangTransport::new(&mut pins), &mut TargetWords);
    pins.handoff_to_keypad();
    result
}
