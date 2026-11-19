#![cfg_attr(not(test), no_std)]

#[path = "gpio_init_plan.rs"]
#[allow(dead_code)]
mod gpio_init_plan;

pub use gpio_init_plan::{Features, GpioPlan};
use gpio_init_plan::{Operation, Port, Register, Update};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RegisterAddress {
    AData,
    ADirection,
    BData,
    BDirection,
    CData,
    CDirection,
}

impl RegisterAddress {
    pub(crate) const fn address(self) -> usize {
        match self {
            Self::AData => 0x4006_0000,
            Self::ADirection => 0x4006_0004,
            Self::BData => 0x4006_0800,
            Self::BDirection => 0x4006_0804,
            Self::CData => 0x4006_1000,
            Self::CDirection => 0x4006_1004,
        }
    }
}

pub(crate) trait RegisterIo {
    fn read(&mut self, register: RegisterAddress) -> u32;
    fn write(&mut self, register: RegisterAddress, value: u32);
}

fn register_address(update: Update) -> RegisterAddress {
    match (update.port, update.register) {
        (Port::A, Register::Data) => RegisterAddress::AData,
        (Port::A, Register::Direction) => RegisterAddress::ADirection,
        (Port::B, Register::Data) => RegisterAddress::BData,
        (Port::B, Register::Direction) => RegisterAddress::BDirection,
        (Port::C, Register::Data) => RegisterAddress::CData,
        (Port::C, Register::Direction) => RegisterAddress::CDirection,
    }
}

pub(crate) fn apply_with_io(io: &mut impl RegisterIo, plan: &GpioPlan) {
    for update in plan.updates() {
        let register = register_address(update);
        let current = io.read(register);
        let updated = match update.operation {
            Operation::SetBits => current | update.mask,
            Operation::ClearBits => current & !update.mask,
        };
        io.write(register, updated);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct VolatileRegisterIo;

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl RegisterIo for VolatileRegisterIo {
    fn read(&mut self, register: RegisterAddress) -> u32 {
        let address = register.address() as *const u32;
        // SAFETY: The caller of the public unsafe entry point guarantees the DP32G030 mapping.
        unsafe { core::ptr::read_volatile(address) }
    }

    fn write(&mut self, register: RegisterAddress, value: u32) {
        let address = register.address() as *mut u32;
        // SAFETY: The caller of the public unsafe entry point guarantees the DP32G030 mapping.
        unsafe { core::ptr::write_volatile(address, value) }
    }
}

/// Applies the GPIO startup plan using volatile 32-bit accesses to the DP32G030 GPIO registers.
///
/// # Safety
///
/// The caller must be executing on a DP32G030 with the GPIOA/B/C mapping below,
/// must have enabled the GPIO peripheral clocks, and must have configured the
/// required pins for GPIO operation. No other context may concurrently modify
/// these DATA or DIR registers during this read-modify-write sequence.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn apply_gpio_startup_plan(plan: &GpioPlan) {
    apply_with_io(&mut VolatileRegisterIo, plan);
}
