#![cfg_attr(not(test), no_std)]

//! GPIOA/PortCon implementation of the I2C bit-flow operations.

use super::{Direction, I2cIo, Level};

pub const GPIOA_DATA: usize = 0x4006_0000;
pub const GPIOA_DIRECTION: usize = 0x4006_0004;
pub const PORTA_INPUT_ENABLE: usize = 0x400b_0100;
pub const PORTA_OPEN_DRAIN: usize = 0x400b_0400;

pub const SCL_MASK: u32 = 1 << 10;
pub const SDA_MASK: u32 = 1 << 11;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Register {
    GpioAData,
    GpioADirection,
    PortAInputEnable,
    PortAOpenDrain,
}

impl Register {
    pub const fn address(self) -> usize {
        match self {
            Self::GpioAData => GPIOA_DATA,
            Self::GpioADirection => GPIOA_DIRECTION,
            Self::PortAInputEnable => PORTA_INPUT_ENABLE,
            Self::PortAOpenDrain => PORTA_OPEN_DRAIN,
        }
    }
}

pub trait RegisterIo {
    fn read(&mut self, register: Register) -> u32;
    fn write(&mut self, register: Register, value: u32);
}

pub trait Delay1us {
    fn delay_1us(&mut self);
}

pub struct GpioI2cIo<R: RegisterIo, D: Delay1us> {
    registers: R,
    delay: D,
    released: bool,
}

impl<R: RegisterIo, D: Delay1us> GpioI2cIo<R, D> {
    pub const fn new(registers: R, delay: D) -> Self {
        Self {
            registers,
            delay,
            released: false,
        }
    }

    fn update(&mut self, register: Register, mask: u32, set: bool) {
        let current = self.registers.read(register);
        let value = if set { current | mask } else { current & !mask };
        self.registers.write(register, value);
    }

    fn set_scl(&mut self, level: Level) {
        self.update(Register::GpioAData, SCL_MASK, level == Level::High);
    }

    fn set_sda(&mut self, level: Level) {
        let current = self.registers.read(Register::GpioAData);
        let value = match level {
            Level::Low => current & !SDA_MASK,
            Level::High => current | SDA_MASK,
        };
        self.registers.write(Register::GpioAData, value);
    }

    fn restore_keypad_mode(&mut self) {
        self.set_sda(Level::High);
        self.update(Register::PortAInputEnable, SDA_MASK, false);
        self.update(Register::PortAOpenDrain, SDA_MASK, false);
        self.update(Register::GpioADirection, SDA_MASK, true);
    }

    /// Restores the board-initialized SDA mode and consumes this bus owner.
    ///
    /// The caller must complete the I2C transaction, including STOP, before
    /// handing these keyboard-shared pins back.
    pub fn handoff_to_keypad(mut self) {
        self.restore_keypad_mode();
        self.released = true;
    }
}

impl<R: RegisterIo, D: Delay1us> I2cIo for GpioI2cIo<R, D> {
    fn scl(&mut self, level: Level) {
        self.set_scl(level);
    }

    fn sda(&mut self, level: Level) {
        self.set_sda(level);
    }

    fn sda_input_enable(&mut self, enabled: bool) {
        self.update(Register::PortAInputEnable, SDA_MASK, enabled);
    }

    fn sda_open_drain(&mut self, enabled: bool) {
        self.update(Register::PortAOpenDrain, SDA_MASK, enabled);
    }

    fn sda_direction(&mut self, direction: Direction) {
        self.update(
            Register::GpioADirection,
            SDA_MASK,
            direction == Direction::Output,
        );
    }

    fn read_sda(&mut self) -> Level {
        if self.registers.read(Register::GpioAData) & SDA_MASK == 0 {
            Level::Low
        } else {
            Level::High
        }
    }

    fn delay_1us(&mut self) {
        self.delay.delay_1us();
    }
}

impl<R: RegisterIo, D: Delay1us> Drop for GpioI2cIo<R, D> {
    fn drop(&mut self) {
        if !self.released {
            self.restore_keypad_mode();
        }
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub struct TargetRegisters(());

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl RegisterIo for TargetRegisters {
    fn read(&mut self, register: Register) -> u32 {
        // SAFETY: `new_target` requires the caller to guarantee the DP32G030 map.
        unsafe { core::ptr::read_volatile(register.address() as *const u32) }
    }

    fn write(&mut self, register: Register, value: u32) {
        // SAFETY: `new_target` requires the caller to guarantee the DP32G030 map.
        unsafe { core::ptr::write_volatile(register.address() as *mut u32, value) }
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl<D: Delay1us> GpioI2cIo<TargetRegisters, D> {
    /// Creates a GPIOA/PortCon bus owner using volatile target register access.
    ///
    /// # Safety
    ///
    /// The caller must be on a DP32G030 with GPIOA and PORTCON accessible at
    /// the documented addresses. The board setup must have enabled the GPIOA
    /// clock, selected GPIOA10/11 in PORTA_SEL1, and set both pins to output
    /// with input-enable and open-drain disabled, as in the pinned
    /// `BOARD_PORTCON_Init` and `BOARD_GPIO_Init`. The caller must exclusively
    /// own PA10/PA11 until this value is handed back or dropped: keypad scans,
    /// interrupts, and other bus users must not access the pins in that period.
    /// The caller must complete the transaction with STOP before handoff or
    /// drop; dropping restores pin configuration but does not stop the bus.
    /// The supplied delay provider is responsible for implementing each
    /// requested one-microsecond delay; this constructor makes no timing claim.
    pub unsafe fn new_target(delay: D) -> Self {
        Self::new(TargetRegisters(()), delay)
    }
}
