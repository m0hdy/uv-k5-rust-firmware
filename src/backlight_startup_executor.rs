#![cfg_attr(not(test), no_std)]

#[allow(dead_code, unused_attributes)]
#[path = "backlight_plan.rs"]
mod backlight_plan;

use backlight_plan::{BacklightInitPlan, BacklightSnapshot};
pub use backlight_plan::{Register, WriteKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RegisterAddress {
    PwmPlus0ClockSource,
    PwmPlus0Period,
    PortBSel0,
    PwmPlus0Generator,
    PwmPlus0Config,
}

impl RegisterAddress {
    pub(crate) const fn from_register(register: Register) -> Self {
        match register {
            Register::PwmPlus0ClockSource => Self::PwmPlus0ClockSource,
            Register::PwmPlus0Period => Self::PwmPlus0Period,
            Register::PortBSel0 => Self::PortBSel0,
            Register::PwmPlus0Generator => Self::PwmPlus0Generator,
            Register::PwmPlus0Config => Self::PwmPlus0Config,
        }
    }

    pub(crate) const fn address(self) -> usize {
        match self {
            Self::PwmPlus0ClockSource => 0x400b_4008,
            Self::PwmPlus0Period => 0x400b_401c,
            Self::PortBSel0 => 0x400b_0008,
            Self::PwmPlus0Generator => 0x400b_4004,
            Self::PwmPlus0Config => 0x400b_4000,
        }
    }
}

pub trait RegisterIo {
    fn read(&mut self, register: Register) -> u32;
    fn write(&mut self, register: Register, value: u32);
}

pub fn apply_with_io(io: &mut impl RegisterIo) {
    let plan = BacklightInitPlan::from_snapshot(BacklightSnapshot {
        pwm_plus0_clksrc: 0,
        portb_sel0: 0,
    });

    for operation in plan.writes() {
        match operation.kind {
            WriteKind::Or { bits } => {
                let current = io.read(operation.register);
                io.write(operation.register, current | bits);
            }
            WriteKind::Clear { mask } => {
                let current = io.read(operation.register);
                io.write(operation.register, current & !mask);
            }
            WriteKind::Set { bits } => {
                let current = io.read(operation.register);
                io.write(operation.register, current | bits);
            }
            WriteKind::Assign => io.write(operation.register, operation.value),
        }
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct VolatileRegisterIo;

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl RegisterIo for VolatileRegisterIo {
    fn read(&mut self, register: Register) -> u32 {
        let address = RegisterAddress::from_register(register).address() as *const u32;
        // SAFETY: The caller of the public unsafe entry point guarantees the DP32G030 mapping.
        unsafe { core::ptr::read_volatile(address) }
    }

    fn write(&mut self, register: Register, value: u32) {
        let address = RegisterAddress::from_register(register).address() as *mut u32;
        // SAFETY: The caller of the public unsafe entry point guarantees the DP32G030 mapping.
        unsafe { core::ptr::write_volatile(address, value) }
    }
}

/// Initializes the backlight PWM using the pinned DP32G030 register sequence.
///
/// # Safety
///
/// The caller must ensure a DP32G030 register map with PWM_PLUS0 CLKSRC,
/// PERIOD, GEN, and CFG at `0x400b4008`, `0x400b401c`, `0x400b4004`, and
/// `0x400b4000`, and PORTB SEL0 at `0x400b0008`. All registers must be
/// accessible for aligned 32-bit reads and writes, and the PWM_PLUS0 clock
/// gate must already be enabled. B6 must be reserved for the backlight. The
/// caller must have exclusive access to these registers, including PORTB
/// SEL0, for the entire sequence.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn initialize_backlight_hardware() {
    apply_with_io(&mut VolatileRegisterIo);
}
