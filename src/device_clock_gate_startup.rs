#![cfg_attr(not(test), no_std)]

//! Isolated model of the default boot device clock gate assignment.

const GPIOA: u32 = 1 << 0;
const GPIOB: u32 = 1 << 1;
const GPIOC: u32 = 1 << 2;
const UART1: u32 = 1 << 7;
const SPI0: u32 = 1 << 10;
const PWM_PLUS0: u32 = 1 << 20;
const SARADC: u32 = 1 << 25;
const CRC: u32 = 1 << 27;
const AES: u32 = 1 << 28;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Register {
    DeviceClockGate,
}

impl Register {
    pub(crate) const fn address(self) -> usize {
        match self {
            Self::DeviceClockGate => 0x4000_0008,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GateValue(u32);

impl GateValue {
    pub(crate) const fn bits(self) -> u32 {
        self.0
    }
}

pub(crate) const DEFAULT_GATES: GateValue =
    GateValue(GPIOA | GPIOB | GPIOC | UART1 | SPI0 | SARADC | CRC | AES | PWM_PLUS0);

pub(crate) trait RegisterWrite {
    fn write(&mut self, register: Register, value: GateValue);
}

pub(crate) fn apply_with_io(io: &mut impl RegisterWrite) {
    io.write(Register::DeviceClockGate, DEFAULT_GATES);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct VolatileRegisterWrite;

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl RegisterWrite for VolatileRegisterWrite {
    fn write(&mut self, register: Register, value: GateValue) {
        // SAFETY: The caller guarantees the register mapping and exclusive ownership.
        unsafe { core::ptr::write_volatile(register.address() as *mut u32, value.bits()) }
    }
}

/// Replaces the DP32G030 device clock gate register with the default boot value.
///
/// # Safety
///
/// The caller must ensure that the DP32G030 device clock gate register is
/// mapped at `0x4000_0008` and accepts 32-bit writes, and must own the clock
/// configuration and register exclusively for the duration of this operation.
/// Peripherals whose gates will be cleared must be idle and safe to clock-gate;
/// no other context may depend on their clocks remaining enabled.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn apply_device_clock_gates() {
    apply_with_io(&mut VolatileRegisterWrite);
}
