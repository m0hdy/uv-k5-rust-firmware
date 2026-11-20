//! Callable SysTick counter-polling delays; not connected to firmware startup.

use crate::{
    i2c_bitbang_flow::target_gpio::Delay1us, st7565_startup_executor::target_spi0::DelayMs,
    systick_timing,
};
use core::num::NonZeroU32;

#[cfg(all(target_arch = "arm", target_os = "none"))]
const SYSTICK_LOAD: usize = 0xe000_e014;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const SYSTICK_VAL: usize = 0xe000_e018;

/// Read-only access to the SysTick LOAD and VAL registers.
pub trait SysTickCounter {
    fn read_load(&mut self) -> u32;
    fn read_value(&mut self) -> u32;
}

/// Delay provider using a free-running SysTick counter and explicit calibration.
pub struct SysTickDelay<R> {
    counter: R,
    ticks_per_microsecond: NonZeroU32,
}

impl<R: SysTickCounter> SysTickDelay<R> {
    /// Creates a delay provider with a caller-verified integer tick rate.
    pub const fn new(counter: R, ticks_per_microsecond: NonZeroU32) -> Self {
        Self {
            counter,
            ticks_per_microsecond,
        }
    }

    /// Waits for the requested microseconds using wrapping `u32` tick arithmetic.
    pub fn delay_us(&mut self, microseconds: u32) {
        self.delay_ticks(delay_us_target_ticks(
            microseconds,
            self.ticks_per_microsecond,
        ));
    }

    fn delay_ticks(&mut self, target_ticks: u32) {
        let reload = self.counter.read_load();
        let mut previous = self.counter.read_value();
        let mut elapsed_ticks = 0u32;

        loop {
            let current = loop {
                let current = self.counter.read_value();
                if current != previous {
                    break current;
                }
            };

            if let Some(elapsed) =
                systick_timing::elapsed_after_sample(elapsed_ticks, reload, previous, current)
            {
                elapsed_ticks = elapsed;
            }
            previous = current;

            if elapsed_ticks >= target_ticks {
                break;
            }
        }
    }
}

impl<R: SysTickCounter> Delay1us for SysTickDelay<R> {
    fn delay_1us(&mut self) {
        self.delay_us(1);
    }
}

impl<R: SysTickCounter> DelayMs for SysTickDelay<R> {
    fn delay_ms(&mut self, milliseconds: u16) {
        self.delay_ticks(delay_ms_target_ticks(
            u32::from(milliseconds),
            self.ticks_per_microsecond,
        ));
    }
}

/// Converts milliseconds to a wrapping tick target using C `u32` arithmetic.
pub const fn delay_ms_target_ticks(milliseconds: u32, ticks_per_microsecond: NonZeroU32) -> u32 {
    milliseconds
        .wrapping_mul(1_000)
        .wrapping_mul(ticks_per_microsecond.get())
}

const fn delay_us_target_ticks(microseconds: u32, ticks_per_microsecond: NonZeroU32) -> u32 {
    microseconds.wrapping_mul(ticks_per_microsecond.get())
}

/// Volatile reader for the Cortex-M SysTick LOAD and VAL registers.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub struct VolatileSysTickCounter(());

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl VolatileSysTickCounter {
    /// Creates the target reader.
    ///
    /// # Safety
    ///
    /// The caller must ensure these addresses map to the target SysTick
    /// registers and that no other code writes LOAD or VAL or reconfigures
    /// SysTick while this provider is used.
    pub const unsafe fn new() -> Self {
        Self(())
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl SysTickCounter for VolatileSysTickCounter {
    fn read_load(&mut self) -> u32 {
        unsafe { core::ptr::read_volatile(SYSTICK_LOAD as *const u32) }
    }

    fn read_value(&mut self) -> u32 {
        unsafe { core::ptr::read_volatile(SYSTICK_VAL as *const u32) }
    }
}
