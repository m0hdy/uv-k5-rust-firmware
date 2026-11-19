//! Fallible I2C execution for an initialized BK1080 receiver.

#[allow(unused_attributes)]
#[path = "bk1080_tuning.rs"]
mod bk1080_tuning;

use crate::eeprom_read_transport::{Acknowledgement, I2cTransport};

pub use bk1080_tuning::FrequencyDeviation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Register {
    PowerConfiguration = 0x02,
    Channel = 0x03,
    SystemConfiguration2 = 0x05,
    Status = 0x07,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiveError<E> {
    NotAcknowledged,
    Transport(E),
}

/// Caller-provided delay in milliseconds. Implementations must wait for the
/// requested interval and must not panic.
pub trait DelayMs {
    fn delay_ms(&mut self, milliseconds: u16);
}

fn write_byte<T: I2cTransport>(transport: &mut T, byte: u8) -> Result<(), ReceiveError<T::Error>> {
    match transport.write_byte(byte) {
        Ok(Acknowledgement::Ack) => Ok(()),
        Ok(Acknowledgement::Nack) => Err(ReceiveError::NotAcknowledged),
        Err(error) => Err(ReceiveError::Transport(error)),
    }
}

fn transaction<T: I2cTransport>(
    transport: &mut T,
    operation: impl FnOnce(&mut T) -> Result<(), ReceiveError<T::Error>>,
) -> Result<(), ReceiveError<T::Error>> {
    transport.start().map_err(ReceiveError::Transport)?;
    if let Err(error) = operation(transport) {
        let _ = transport.stop();
        return Err(error);
    }
    transport.stop().map_err(ReceiveError::Transport)
}

pub fn write_register<T: I2cTransport>(
    transport: &mut T,
    register: Register,
    value: u16,
) -> Result<(), ReceiveError<T::Error>> {
    transaction(transport, |transport| {
        for byte in [0x80, (register as u8) << 1, (value >> 8) as u8, value as u8] {
            write_byte(transport, byte)?;
        }
        Ok(())
    })
}

pub fn read_register<T: I2cTransport>(
    transport: &mut T,
    register: Register,
) -> Result<u16, ReceiveError<T::Error>> {
    let mut value = [0; 2];
    transaction(transport, |transport| {
        write_byte(transport, 0x80)?;
        write_byte(transport, ((register as u8) << 1) | 1)?;
        transport
            .read_buffer(&mut value)
            .map_err(ReceiveError::Transport)
    })?;
    Ok(u16::from_be_bytes(value))
}

pub fn set_frequency<T: I2cTransport, D: DelayMs>(
    transport: &mut T,
    delay: &mut D,
    frequency: u16,
    band: u8,
) -> Result<(), ReceiveError<T::Error>> {
    let configuration = read_register(transport, Register::SystemConfiguration2)?;
    for command in bk1080_tuning::TunePlan::new(frequency, band, configuration).commands() {
        match command {
            bk1080_tuning::Command::WriteRegister { register, value } => {
                let register = match register {
                    bk1080_tuning::Register::PowerConfiguration => Register::PowerConfiguration,
                    bk1080_tuning::Register::Channel => Register::Channel,
                    bk1080_tuning::Register::SystemConfiguration2 => Register::SystemConfiguration2,
                };
                write_register(transport, register, value)?;
            }
            bk1080_tuning::Command::WaitMs(milliseconds) => delay.delay_ms(milliseconds),
        }
    }
    Ok(())
}

pub fn set_muted<T: I2cTransport>(
    transport: &mut T,
    muted: bool,
) -> Result<(), ReceiveError<T::Error>> {
    let command = bk1080_tuning::mute_command(muted);
    let value = match command {
        bk1080_tuning::Command::WriteRegister { value, .. } => value,
        bk1080_tuning::Command::WaitMs(_) => unreachable!("mute command has no delay"),
    };
    write_register(transport, Register::PowerConfiguration, value)
}

pub fn read_frequency_deviation<T: I2cTransport>(
    transport: &mut T,
    base_frequency: u16,
) -> Result<FrequencyDeviation, ReceiveError<T::Error>> {
    let register_07 = read_register(transport, Register::Status)?;
    Ok(bk1080_tuning::frequency_deviation(
        base_frequency,
        register_07,
    ))
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
unsafe fn with_target_transport<D, R>(
    delay: D,
    operation: impl FnOnce(
        &mut crate::eeprom_i2c_bridge::I2cBitbangTransport<
            '_,
            crate::i2c_bitbang_flow::target_gpio::GpioI2cIo<
                crate::i2c_bitbang_flow::target_gpio::TargetRegisters,
                D,
            >,
        >,
    ) -> R,
) -> R
where
    D: crate::i2c_bitbang_flow::target_gpio::Delay1us,
{
    use crate::eeprom_i2c_bridge::I2cBitbangTransport;
    use crate::i2c_bitbang_flow::target_gpio::GpioI2cIo;

    // SAFETY: the unsafe target entry points require the caller to guarantee
    // the target register mapping, board pin setup, and exclusive ownership.
    let mut pins = unsafe { GpioI2cIo::new_target(delay) };
    let result = {
        let mut transport = I2cBitbangTransport::new(&mut pins);
        operation(&mut transport)
    };
    pins.handoff_to_keypad();
    result
}

/// Tunes an initialized receiver through the target GPIOA10/11 I2C owner.
///
/// # Safety
///
/// The caller must provide a DP32G030 with GPIOA and PORTCON mapped and the
/// GPIOA clock enabled. PA10/PA11 must be muxed to GPIOA10/GPIOA11 and
/// initialized as GPIO outputs, with input-enable and open-drain disabled on
/// both pins.
/// The receiver must already be initialized and powered. The caller must
/// exclusively own the I2C bus and keypad pins, and serialize the entire
/// operation—including the 10 ms delay, transaction cleanup, pin handoff, and
/// drop—against every writer to GPIOA DATA/DIR and PORTA input-enable/open-drain,
/// including interrupts and writers to unrelated bits. The supplied one-
/// microsecond delay must be accurate, infallible, non-panicking, and must not
/// access those shared registers. The millisecond provider must be accurate,
/// infallible, non-panicking for the requested 10 ms delay, and must not access
/// the shared GPIO registers.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn set_frequency_mmio<D, M>(
    frequency: u16,
    band: u8,
    delay_1us: D,
    delay_ms: &mut M,
) -> Result<(), ReceiveError<core::convert::Infallible>>
where
    D: crate::i2c_bitbang_flow::target_gpio::Delay1us,
    M: DelayMs,
{
    // SAFETY: the caller upholds the mapping, setup, timing, and ownership
    // requirements documented above for this complete operation.
    unsafe {
        with_target_transport(delay_1us, |transport| {
            set_frequency(transport, delay_ms, frequency, band)
        })
    }
}

/// Mutes or unmutes an initialized receiver through target GPIOA10/11.
///
/// # Safety
///
/// The caller must provide the mapped DP32G030 GPIOA/PORTCON registers and
/// enabled GPIOA clock; PA10/PA11 must be muxed as GPIO outputs with
/// input-enable and open-drain disabled on both pins. The receiver must already
/// be initialized and powered. The caller must exclusively own the I2C bus and
/// keypad pins, and serialize the full operation, cleanup, handoff, and drop
/// against every GPIOA DATA/DIR and PORTA input-enable/open-drain writer, including
/// interrupts and unrelated-bit writers. The supplied one-microsecond delay
/// must be accurate, infallible, non-panicking, and must not access those
/// shared registers.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn set_muted_mmio<D>(
    muted: bool,
    delay_1us: D,
) -> Result<(), ReceiveError<core::convert::Infallible>>
where
    D: crate::i2c_bitbang_flow::target_gpio::Delay1us,
{
    // SAFETY: the caller upholds the mapping, setup, timing, and ownership
    // requirements documented above for this complete operation.
    unsafe { with_target_transport(delay_1us, |transport| set_muted(transport, muted)) }
}

/// Reads the deviation register of an initialized receiver through GPIOA10/11.
///
/// # Safety
///
/// The caller must provide the mapped DP32G030 GPIOA/PORTCON registers and
/// enabled GPIOA clock; PA10/PA11 must be muxed as GPIO outputs with
/// input-enable and open-drain disabled on both pins. The receiver must already
/// be initialized and powered. The caller must exclusively own the I2C bus and
/// keypad pins, and serialize the full operation, cleanup, handoff, and drop
/// against every GPIOA DATA/DIR and PORTA input-enable/open-drain writer, including
/// interrupts and unrelated-bit writers. The supplied one-microsecond delay
/// must be accurate, infallible, non-panicking, and must not access those
/// shared registers.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn read_frequency_deviation_mmio<D>(
    base_frequency: u16,
    delay_1us: D,
) -> Result<FrequencyDeviation, ReceiveError<core::convert::Infallible>>
where
    D: crate::i2c_bitbang_flow::target_gpio::Delay1us,
{
    // SAFETY: the caller upholds the mapping, setup, timing, and ownership
    // requirements documented above for this complete operation.
    unsafe {
        with_target_transport(delay_1us, |transport| {
            read_frequency_deviation(transport, base_frequency)
        })
    }
}
