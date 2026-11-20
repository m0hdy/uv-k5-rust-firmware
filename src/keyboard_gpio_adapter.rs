#![cfg_attr(not(test), no_std)]

//! GPIOA-backed keypad polling through the existing keyboard and I2C flows.

use crate::i2c_bitbang_flow::{
    self, Direction, I2cIo, Level,
    target_gpio::{Delay1us, Register, RegisterIo, SCL_MASK, SDA_MASK},
};
use crate::keyboard_poll_flow::{KeyboardPollFlow, KeypadEffects, MatrixIo};

struct GpioBackend<R, D> {
    registers: R,
    delay: D,
}

/// Retained keyboard polling over one exclusive GPIOA and delay backend.
pub struct KeyboardGpioAdapter<R, D> {
    gpio: GpioBackend<R, D>,
    flow: KeyboardPollFlow,
}

impl<R: RegisterIo, D: Delay1us> KeyboardGpioAdapter<R, D> {
    pub const fn new(registers: R, delay: D) -> Self {
        Self {
            gpio: GpioBackend { registers, delay },
            flow: KeyboardPollFlow::new(),
        }
    }

    /// Performs one poll. The caller supplies the cadence and application effects.
    pub fn poll(&mut self, effects: &mut impl KeypadEffects) {
        self.flow.poll(&mut self.gpio, effects);
    }
}

impl<R: RegisterIo, D: Delay1us> MatrixIo for GpioBackend<R, D> {
    fn update_gpioa(&mut self, and_mask: u32, or_mask: u32) {
        let current = self.registers.read(Register::GpioAData);
        let preservation_mask = and_mask | 0xffff_0000;
        self.registers
            .write(Register::GpioAData, (current & preservation_mask) | or_mask);
    }

    fn read_gpioa(&mut self) -> u32 {
        self.registers.read(Register::GpioAData)
    }

    fn delay_1us(&mut self) {
        self.delay.delay_1us();
    }

    fn i2c_stop(&mut self) {
        let mut io = BorrowedI2cIo { gpio: self };
        i2c_bitbang_flow::stop(&mut io);
    }
}

struct BorrowedI2cIo<'a, R, D> {
    gpio: &'a mut GpioBackend<R, D>,
}

impl<R: RegisterIo, D: Delay1us> BorrowedI2cIo<'_, R, D> {
    fn update(&mut self, register: Register, mask: u32, set: bool) {
        let current = self.gpio.registers.read(register);
        let value = if set { current | mask } else { current & !mask };
        self.gpio.registers.write(register, value);
    }
}

impl<R: RegisterIo, D: Delay1us> I2cIo for BorrowedI2cIo<'_, R, D> {
    fn scl(&mut self, level: Level) {
        self.update(Register::GpioAData, SCL_MASK, level == Level::High);
    }

    fn sda(&mut self, level: Level) {
        self.update(Register::GpioAData, SDA_MASK, level == Level::High);
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
        if self.gpio.registers.read(Register::GpioAData) & SDA_MASK == 0 {
            Level::Low
        } else {
            Level::High
        }
    }

    fn delay_1us(&mut self) {
        self.gpio.delay.delay_1us();
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
impl<D: Delay1us> KeyboardGpioAdapter<TargetRegisters, D> {
    /// Creates a keypad poller using volatile GPIOA/PortA register access.
    ///
    /// # Safety
    ///
    /// The caller must be on a DP32G030 with GPIOA and PORTA registers
    /// accessible at the documented addresses. GPIOA's clock must be enabled,
    /// PA3-PA6 and PA10-PA13 muxed to GPIO, PA3-PA6 configured as inputs, and
    /// PA10-PA13 configured as outputs. PORTA input-enable, pulls, and open
    /// drain must match the board's keypad setup; PA10/PA11 must be outputs
    /// with input-enable and open-drain disabled before polling.
    ///
    /// The caller must grant exclusive keypad, I2C, and voice-pin ownership to
    /// this adapter and serialize its polls against every writer to GPIOA
    /// DATA/DIR and PORTA input-enable/open-drain, including interrupts and
    /// writers that modify unrelated pins. Each update uses a full-register
    /// read-modify-write. The supplied delay must be accurate for each
    /// requested microsecond, infallible, and must not access these shared
    /// registers. These requirements extend through each complete poll,
    /// including STOP and voice cleanup.
    ///
    /// The backend operations are infallible, so cleanup is guaranteed on
    /// normal return only. Panics in a delay or other supplied operation do
    /// not provide cleanup; no unwind guarantee is made.
    pub unsafe fn new_target(delay: D) -> Self {
        Self::new(TargetRegisters(()), delay)
    }
}
