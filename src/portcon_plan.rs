#![cfg_attr(not(test), no_std)]

//! Offline model of the initial `BOARD_PORTCON_Init` SEL writes at pinned
//! reference commit `7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`
//! ([`board.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/board.c)
//! and [DP32G030 PORTCON fields](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h)).
//! The plan records independent read-modify-write operations and their values
//! from supplied snapshots; it performs no MMIO or device access.

const PORTA_SEL0_A3_TO_A6_MASK: u32 = 0x0fff_f000;
const PORTA_SEL0_A7_UART1_TX: u32 = 1 << 28;
const PORTA_SEL1_A10_TO_A13_MASK: u32 = 0x00ff_ff00;
const PORTA_SEL1_A8_UART1_RX: u32 = 1;
const PORTA_SEL1_A9_SARADC_CH4: u32 = 5 << 4;
const PORTA_SEL1_A14_SARADC_CH9: u32 = 5 << 24;
const PORTB_SEL0_B7_MASK: u32 = 0xf000_0000;
const PORTB_SEL0_B7_SPI0_SSN: u32 = 1 << 28;
const PORTB_SEL1_B9_B11_B14_B15_MASK: u32 = 0xff00_f0f0;
const PORTB_SEL1_B8_SPI0_CLK: u32 = 1;
const PORTB_SEL1_B10_SPI0_MOSI: u32 = 1 << 8;
const PORTB_SEL1_B11_SWDIO: u32 = 1 << 12;
const PORTB_SEL1_B14_SWCLK: u32 = 1 << 24;
const PORTC_SEL0_C0_TO_C5_MASK: u32 = 0x00ff_ffff;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Register {
    PortASel0,
    PortASel1,
    PortBSel0,
    PortBSel1,
    PortCSel0,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteKind {
    Clear { mask: u32 },
    Set { bits: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortconWrite {
    pub register: Register,
    pub kind: WriteKind,
    pub value: u32,
}

impl PortconWrite {
    const fn clear(register: Register, before: u32, mask: u32) -> Self {
        Self {
            register,
            kind: WriteKind::Clear { mask },
            value: before & !mask,
        }
    }

    const fn set(register: Register, before: u32, bits: u32) -> Self {
        Self {
            register,
            kind: WriteKind::Set { bits },
            value: before | bits,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Features {
    pub enable_swd: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortconSnapshot {
    pub porta_sel0: u32,
    pub porta_sel1: u32,
    pub portb_sel0: u32,
    pub portb_sel1: u32,
    pub portc_sel0: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortconPlan {
    writes: [PortconWrite; 9],
}

impl PortconPlan {
    pub const fn from_snapshot(snapshot: PortconSnapshot, features: Features) -> Self {
        let porta_sel0_clear = PortconWrite::clear(
            Register::PortASel0,
            snapshot.porta_sel0,
            PORTA_SEL0_A3_TO_A6_MASK,
        );
        let porta_sel0_set = PortconWrite::set(
            Register::PortASel0,
            porta_sel0_clear.value,
            PORTA_SEL0_A7_UART1_TX,
        );
        let porta_sel1_clear = PortconWrite::clear(
            Register::PortASel1,
            snapshot.porta_sel1,
            PORTA_SEL1_A10_TO_A13_MASK,
        );
        let porta_sel1_set = PortconWrite::set(
            Register::PortASel1,
            porta_sel1_clear.value,
            PORTA_SEL1_A8_UART1_RX | PORTA_SEL1_A9_SARADC_CH4 | PORTA_SEL1_A14_SARADC_CH9,
        );
        let portb_sel0_clear =
            PortconWrite::clear(Register::PortBSel0, snapshot.portb_sel0, PORTB_SEL0_B7_MASK);
        let portb_sel0_set = PortconWrite::set(
            Register::PortBSel0,
            portb_sel0_clear.value,
            PORTB_SEL0_B7_SPI0_SSN,
        );
        let portb_sel1_clear = PortconWrite::clear(
            Register::PortBSel1,
            snapshot.portb_sel1,
            PORTB_SEL1_B9_B11_B14_B15_MASK,
        );
        let portb_sel1_bits = PORTB_SEL1_B8_SPI0_CLK
            | PORTB_SEL1_B10_SPI0_MOSI
            | if features.enable_swd {
                PORTB_SEL1_B11_SWDIO | PORTB_SEL1_B14_SWCLK
            } else {
                0
            };
        let portb_sel1_set =
            PortconWrite::set(Register::PortBSel1, portb_sel1_clear.value, portb_sel1_bits);
        let portc_sel0_clear = PortconWrite::clear(
            Register::PortCSel0,
            snapshot.portc_sel0,
            PORTC_SEL0_C0_TO_C5_MASK,
        );

        Self {
            writes: [
                porta_sel0_clear,
                porta_sel0_set,
                porta_sel1_clear,
                porta_sel1_set,
                portb_sel0_clear,
                portb_sel0_set,
                portb_sel1_clear,
                portb_sel1_set,
                portc_sel0_clear,
            ],
        }
    }

    pub fn writes(&self) -> impl Iterator<Item = PortconWrite> + '_ {
        self.writes.iter().copied()
    }
}
