#![no_std]

//! Offline model of `SPI0_Init` and `SPI_ToggleMasterMode` at pinned reference
//! commit `7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`
//! ([`spi.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/spi.c),
//! [SPI fields](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/spi.h),
//! and [SYSCON fields](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/syscon.h)).
//! This independently authored, allocation-free plan records logical register
//! writes from supplied snapshots; it performs no MMIO and models no hardware
//! side effects.
//!
//! The pinned configuration does not clear `TF_CLR`: that field is absent from
//! the control-register clear mask, and its configured value is zero.

const SPI_CR_SPR_MASK: u32 = 0b111;
pub(super) const SPI_CR_SPE_MASK: u32 = 1 << 3;
const SPI_CR_CPHA_MASK: u32 = 1 << 4;
const SPI_CR_CPOL_MASK: u32 = 1 << 5;
const SPI_CR_MSTR_MASK: u32 = 1 << 6;
const SPI_CR_LSB_MASK: u32 = 1 << 7;
const SPI_CR_MSR_SSN_MASK: u32 = 1 << 12;
const SPI_CR_RF_CLR_MASK: u32 = 1 << 15;
pub(super) const SYSCON_DEV_CLK_GATE_SPI0_MASK: u32 = 1 << 10;

pub(super) const SPI0_CONFIGURATION_CLEAR_MASK: u32 = SPI_CR_SPR_MASK
    | SPI_CR_CPHA_MASK
    | SPI_CR_CPOL_MASK
    | SPI_CR_MSTR_MASK
    | SPI_CR_LSB_MASK
    | SPI_CR_RF_CLR_MASK;
pub(super) const SPI0_CONFIGURATION_BITS: u32 =
    2 | SPI_CR_CPHA_MASK | SPI_CR_CPOL_MASK | SPI_CR_MSTR_MASK;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Register {
    Control,
    InterruptEnable,
    DeviceClockGate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogicalWrite {
    pub register: Register,
    pub value: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Spi0Snapshot {
    pub control: u32,
    pub interrupt_enable: u32,
    pub device_clock_gate: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Spi0InitPlan {
    writes: [LogicalWrite; 6],
}

impl Spi0InitPlan {
    pub const fn from_snapshot(snapshot: Spi0Snapshot) -> Self {
        let first_disable = snapshot.control & !SPI_CR_SPE_MASK;
        let configured_control =
            (first_disable & !SPI0_CONFIGURATION_CLEAR_MASK) | SPI0_CONFIGURATION_BITS;
        Self {
            writes: [
                LogicalWrite {
                    register: Register::Control,
                    value: first_disable,
                },
                LogicalWrite {
                    register: Register::DeviceClockGate,
                    value: (snapshot.device_clock_gate & !SYSCON_DEV_CLK_GATE_SPI0_MASK)
                        | SYSCON_DEV_CLK_GATE_SPI0_MASK,
                },
                LogicalWrite {
                    register: Register::Control,
                    value: first_disable,
                },
                LogicalWrite {
                    register: Register::Control,
                    value: configured_control,
                },
                LogicalWrite {
                    register: Register::InterruptEnable,
                    value: 0,
                },
                LogicalWrite {
                    register: Register::Control,
                    value: configured_control | SPI_CR_SPE_MASK,
                },
            ],
        }
    }

    pub fn writes(&self) -> impl Iterator<Item = LogicalWrite> + '_ {
        self.writes.iter().copied()
    }
}

pub const fn toggle_master_mode(control: u32, is_master: bool) -> LogicalWrite {
    let value = if is_master {
        (control & !SPI_CR_MSR_SSN_MASK) | SPI_CR_MSR_SSN_MASK
    } else {
        control & !SPI_CR_MSR_SSN_MASK
    };
    LogicalWrite {
        register: Register::Control,
        value,
    }
}
