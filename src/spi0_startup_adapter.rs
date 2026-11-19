#![cfg_attr(not(test), no_std)]

#[allow(dead_code, unused_attributes)]
#[path = "spi0_plan.rs"]
mod spi0_plan;

use spi0_plan::{
    SPI_CR_SPE_MASK, SPI0_CONFIGURATION_BITS, SPI0_CONFIGURATION_CLEAR_MASK,
    SYSCON_DEV_CLK_GATE_SPI0_MASK,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RegisterAddress {
    Spi0Control,
    Spi0InterruptEnable,
    DeviceClockGate,
}

impl RegisterAddress {
    pub(crate) const fn address(self) -> usize {
        match self {
            Self::Spi0Control => 0x400b_8000,
            Self::Spi0InterruptEnable => 0x400b_8010,
            Self::DeviceClockGate => 0x4000_0008,
        }
    }
}

pub(crate) trait RegisterIo {
    fn read(&mut self, register: RegisterAddress) -> u32;
    fn write(&mut self, register: RegisterAddress, value: u32);
}

fn disable(io: &mut impl RegisterIo) {
    let control = io.read(RegisterAddress::Spi0Control);
    io.write(RegisterAddress::Spi0Control, control & !SPI_CR_SPE_MASK);
}

pub(crate) fn apply_with_io(io: &mut impl RegisterIo) {
    disable(io);

    let device_clock_gate = io.read(RegisterAddress::DeviceClockGate);
    io.write(
        RegisterAddress::DeviceClockGate,
        (device_clock_gate & !SYSCON_DEV_CLK_GATE_SPI0_MASK) | SYSCON_DEV_CLK_GATE_SPI0_MASK,
    );

    disable(io);

    let control = io.read(RegisterAddress::Spi0Control);
    io.write(
        RegisterAddress::Spi0Control,
        (control & !SPI0_CONFIGURATION_CLEAR_MASK) | SPI0_CONFIGURATION_BITS,
    );

    io.write(RegisterAddress::Spi0InterruptEnable, 0);
    let _unexpected_interrupt_enable = io.read(RegisterAddress::Spi0InterruptEnable);

    let control = io.read(RegisterAddress::Spi0Control);
    io.write(RegisterAddress::Spi0Control, control | SPI_CR_SPE_MASK);
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

/// Initializes SPI0 using the pinned DP32G030 register access sequence.
///
/// # Safety
///
/// The caller must ensure a DP32G030 register map with SPI0 CR/IE at
/// `0x400b8000`/`0x400b8010` and `SYSCON_DEV_CLK_GATE` at `0x40000008`.
/// SPI0 and SYSCON must be accessible before entry; specifically, SPI0's
/// clock gate must already be enabled because CR is accessed before the
/// adapter's clock-gate read-modify-write. The SPI0 display pins (B7 SSN,
/// B8 CLK, and B10 MOSI) must already be muxed. The caller must have exclusive
/// ownership of SPI0 CR, SPI0 IE, and `SYSCON_DEV_CLK_GATE` for this call.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn initialize_spi0() {
    apply_with_io(&mut VolatileRegisterIo);
}
