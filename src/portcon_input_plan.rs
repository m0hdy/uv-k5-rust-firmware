#![cfg_attr(not(test), no_std)]

//! Offline model of the input, pull, and open-drain writes in
//! `BOARD_PORTCON_Init` at pinned reference commit
//! `7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`. The plan records ordered
//! read-modify-write results from supplied snapshots; it performs no MMIO.

const PORTA_INPUTS_A3_TO_A6: u32 = 0x0000_0078;
const PORTA_INPUTS_A10_TO_A13: u32 = 0x0000_3c00;
const PORTA_INPUTS_A3_TO_A6_AND_A10_TO_A13: u32 = 0x0000_3c78;
const PORTB_INPUTS_B6_B7_B8_B9_B10_B11_B15: u32 = 0x0000_8fc0;
const PORTB_INPUTS_B6_B7_B8_B9_B10_B15: u32 = 0x0000_87c0;
const PORTB_INPUTS_B6_B9_B11_B14_B15: u32 = 0x0000_ca40;
const PORTB_INPUTS_B6_B9_B11_B15: u32 = 0x0000_8a40;
const PORTC_INPUTS_C0_TO_C4: u32 = 0x0000_001f;
const PORTC_INPUTS_C0_TO_C5: u32 = 0x0000_003f;
const PORTC_INPUT_C5: u32 = 0x0000_0020;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Register {
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
pub struct PortconInputSnapshot {
    pub porta_ie: u32,
    pub porta_pu: u32,
    pub porta_pd: u32,
    pub porta_od: u32,
    pub portb_ie: u32,
    pub portb_pu: u32,
    pub portb_pd: u32,
    pub portb_od: u32,
    pub portc_ie: u32,
    pub portc_pu: u32,
    pub portc_pd: u32,
    pub portc_od: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortconInputPlan {
    writes: [PortconWrite; 20],
}

impl PortconInputPlan {
    pub const fn from_snapshot(snapshot: PortconInputSnapshot, features: Features) -> Self {
        let porta_ie_set =
            PortconWrite::set(Register::AInputEnable, snapshot.porta_ie, 0x0000_0178);
        let porta_ie_clear = PortconWrite::clear(
            Register::AInputEnable,
            porta_ie_set.value,
            PORTA_INPUTS_A10_TO_A13,
        );
        let porta_pu_set =
            PortconWrite::set(Register::APullUp, snapshot.porta_pu, PORTA_INPUTS_A3_TO_A6);
        let porta_pu_clear = PortconWrite::clear(
            Register::APullUp,
            porta_pu_set.value,
            PORTA_INPUTS_A10_TO_A13,
        );
        let porta_pd_clear = PortconWrite::clear(
            Register::APullDown,
            snapshot.porta_pd,
            PORTA_INPUTS_A3_TO_A6_AND_A10_TO_A13,
        );
        let porta_od_set = PortconWrite::set(
            Register::AOpenDrain,
            snapshot.porta_od,
            PORTA_INPUTS_A3_TO_A6,
        );
        let porta_od_clear = PortconWrite::clear(
            Register::AOpenDrain,
            porta_od_set.value,
            PORTA_INPUTS_A10_TO_A13,
        );

        let portb_ie_set = PortconWrite::set(Register::BInputEnable, snapshot.portb_ie, 1 << 14);
        let portb_ie_clear = PortconWrite::clear(
            Register::BInputEnable,
            portb_ie_set.value,
            if features.enable_swd {
                PORTB_INPUTS_B6_B7_B8_B9_B10_B15
            } else {
                PORTB_INPUTS_B6_B7_B8_B9_B10_B11_B15
            },
        );
        let portb_pu_clear = PortconWrite::clear(
            Register::BPullUp,
            snapshot.portb_pu,
            PORTB_INPUTS_B6_B9_B11_B14_B15,
        );
        let portb_pd_clear = PortconWrite::clear(
            Register::BPullDown,
            snapshot.portb_pd,
            PORTB_INPUTS_B6_B9_B11_B14_B15,
        );
        let portb_od_clear = PortconWrite::clear(
            Register::BOpenDrain,
            snapshot.portb_od,
            PORTB_INPUTS_B6_B9_B11_B15,
        );
        let portb_od_set = PortconWrite::set(Register::BOpenDrain, portb_od_clear.value, 1 << 14);

        let portc_ie_set =
            PortconWrite::set(Register::CInputEnable, snapshot.portc_ie, PORTC_INPUT_C5);
        let portc_ie_clear = PortconWrite::clear(
            Register::CInputEnable,
            portc_ie_set.value,
            PORTC_INPUTS_C0_TO_C4,
        );
        let portc_pu_set = PortconWrite::set(Register::CPullUp, snapshot.portc_pu, PORTC_INPUT_C5);
        let portc_pu_clear =
            PortconWrite::clear(Register::CPullUp, portc_pu_set.value, PORTC_INPUTS_C0_TO_C4);
        let portc_pd_clear = PortconWrite::clear(
            Register::CPullDown,
            snapshot.portc_pd,
            PORTC_INPUTS_C0_TO_C5,
        );
        let portc_od_clear = PortconWrite::clear(
            Register::COpenDrain,
            snapshot.portc_od,
            PORTC_INPUTS_C0_TO_C4,
        );
        let portc_od_set =
            PortconWrite::set(Register::COpenDrain, portc_od_clear.value, PORTC_INPUT_C5);

        Self {
            writes: [
                porta_ie_set,
                porta_ie_clear,
                porta_pu_set,
                porta_pu_clear,
                porta_pd_clear,
                porta_od_set,
                porta_od_clear,
                portb_ie_set,
                portb_ie_clear,
                portb_pu_clear,
                portb_pd_clear,
                portb_od_clear,
                portb_od_set,
                portc_ie_set,
                portc_ie_clear,
                portc_pu_set,
                portc_pu_clear,
                portc_pd_clear,
                portc_od_clear,
                portc_od_set,
            ],
        }
    }

    pub fn writes(&self) -> impl Iterator<Item = PortconWrite> + '_ {
        self.writes.iter().copied()
    }
}
