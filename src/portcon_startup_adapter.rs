#![cfg_attr(not(test), no_std)]

#[path = "portcon_input_plan.rs"]
#[allow(dead_code, unused_attributes)]
mod input_plan;
#[path = "portcon_plan.rs"]
#[allow(dead_code, unused_attributes)]
mod sel_plan;

use input_plan::{
    PortconInputPlan, PortconInputSnapshot, PortconWrite as InputWrite, Register as InputRegister,
    WriteKind as InputWriteKind,
};
pub(crate) use sel_plan::Features;
use sel_plan::WriteKind as SelWriteKind;
use sel_plan::{PortconPlan, PortconSnapshot, PortconWrite as SelWrite, Register as SelRegister};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RegisterAddress {
    PortASel0,
    PortASel1,
    PortBSel0,
    PortBSel1,
    PortCSel0,
    AInputEnable,
    BInputEnable,
    CInputEnable,
    APullUp,
    BPullUp,
    CPullUp,
    APullDown,
    BPullDown,
    CPullDown,
    AOpenDrain,
    BOpenDrain,
    COpenDrain,
}

impl RegisterAddress {
    pub(crate) const fn address(self) -> usize {
        match self {
            Self::PortASel0 => 0x400b_0000,
            Self::PortASel1 => 0x400b_0004,
            Self::PortBSel0 => 0x400b_0008,
            Self::PortBSel1 => 0x400b_000c,
            Self::PortCSel0 => 0x400b_0010,
            Self::AInputEnable => 0x400b_0100,
            Self::BInputEnable => 0x400b_0104,
            Self::CInputEnable => 0x400b_0108,
            Self::APullUp => 0x400b_0200,
            Self::BPullUp => 0x400b_0204,
            Self::CPullUp => 0x400b_0208,
            Self::APullDown => 0x400b_0300,
            Self::BPullDown => 0x400b_0304,
            Self::CPullDown => 0x400b_0308,
            Self::AOpenDrain => 0x400b_0400,
            Self::BOpenDrain => 0x400b_0404,
            Self::COpenDrain => 0x400b_0408,
        }
    }
}

pub(crate) trait RegisterIo {
    fn read(&mut self, register: RegisterAddress) -> u32;
    fn write(&mut self, register: RegisterAddress, value: u32);
}

fn sel_register(register: SelRegister) -> RegisterAddress {
    match register {
        SelRegister::PortASel0 => RegisterAddress::PortASel0,
        SelRegister::PortASel1 => RegisterAddress::PortASel1,
        SelRegister::PortBSel0 => RegisterAddress::PortBSel0,
        SelRegister::PortBSel1 => RegisterAddress::PortBSel1,
        SelRegister::PortCSel0 => RegisterAddress::PortCSel0,
    }
}

fn input_register(register: InputRegister) -> RegisterAddress {
    match register {
        InputRegister::AInputEnable => RegisterAddress::AInputEnable,
        InputRegister::BInputEnable => RegisterAddress::BInputEnable,
        InputRegister::CInputEnable => RegisterAddress::CInputEnable,
        InputRegister::APullUp => RegisterAddress::APullUp,
        InputRegister::BPullUp => RegisterAddress::BPullUp,
        InputRegister::CPullUp => RegisterAddress::CPullUp,
        InputRegister::APullDown => RegisterAddress::APullDown,
        InputRegister::BPullDown => RegisterAddress::BPullDown,
        InputRegister::CPullDown => RegisterAddress::CPullDown,
        InputRegister::AOpenDrain => RegisterAddress::AOpenDrain,
        InputRegister::BOpenDrain => RegisterAddress::BOpenDrain,
        InputRegister::COpenDrain => RegisterAddress::COpenDrain,
    }
}

fn apply_rmw(io: &mut impl RegisterIo, register: RegisterAddress, clear_mask: u32, set_bits: u32) {
    let current = io.read(register);
    io.write(register, (current & !clear_mask) | set_bits);
}

fn apply_sel_write(io: &mut impl RegisterIo, write: SelWrite) {
    let (clear_mask, set_bits) = match write.kind {
        SelWriteKind::Clear { mask } => (mask, 0),
        SelWriteKind::Set { bits } => (0, bits),
    };
    apply_rmw(io, sel_register(write.register), clear_mask, set_bits);
}

fn apply_input_write(io: &mut impl RegisterIo, write: InputWrite) {
    let (clear_mask, set_bits) = match write.kind {
        InputWriteKind::Clear { mask } => (mask, 0),
        InputWriteKind::Set { bits } => (0, bits),
    };
    apply_rmw(io, input_register(write.register), clear_mask, set_bits);
}

pub(crate) fn apply_with_io(io: &mut impl RegisterIo, features: Features) {
    let sel_plan = PortconPlan::from_snapshot(
        PortconSnapshot {
            porta_sel0: 0,
            porta_sel1: 0,
            portb_sel0: 0,
            portb_sel1: 0,
            portc_sel0: 0,
        },
        features,
    );
    for write in sel_plan.writes() {
        apply_sel_write(io, write);
    }

    let input_plan = PortconInputPlan::from_snapshot(
        PortconInputSnapshot {
            porta_ie: 0,
            porta_pu: 0,
            porta_pd: 0,
            porta_od: 0,
            portb_ie: 0,
            portb_pu: 0,
            portb_pd: 0,
            portb_od: 0,
            portc_ie: 0,
            portc_pu: 0,
            portc_pd: 0,
            portc_od: 0,
        },
        input_plan::Features {
            enable_swd: features.enable_swd,
        },
    );
    for write in input_plan.writes() {
        apply_input_write(io, write);
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

/// Applies the PortCon SEL, input-enable, pull, and open-drain startup plans.
///
/// # Safety
///
/// The caller must ensure that the target is a DP32G030 with the PORTCON
/// register mapping defined by the pinned `bsp/dp32g030/portcon.h`, and that
/// the mapped registers are accessible for aligned 32-bit reads and writes.
/// This routine must have exclusive startup ownership of every affected
/// PORTCON register: no interrupt, thread, or other code may modify them
/// during the read-modify-write sequence.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn apply_portcon_startup_plan(features: Features) {
    apply_with_io(&mut VolatileRegisterIo, features);
}
