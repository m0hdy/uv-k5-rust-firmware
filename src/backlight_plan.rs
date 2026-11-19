#![cfg_attr(not(test), no_std)]

//! Offline trace of the backlight PWM initialization and compare arithmetic
//! from `egzumer/uv-k5-firmware-custom@7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`.
//! This records source operations over supplied snapshots; it performs no
//! MMIO and makes no claim about physical PWM output.

const CLOCK_SOURCE_BITS: u32 = ((48_000_000 / 1024 / 1000) as u32) << 16;
const PORTB_SEL0_B6_MASK: u32 = 0x0f00_0000;
const PORTB_SEL0_B6_PWMP0_CH0: u32 = 0x0100_0000;
const PWM_PLUS0_GEN: u32 = (1 << 24) | (1 << 16);
const PWM_PLUS0_CFG: u32 = (1 << 2) | 1;

pub const MIN_BRIGHTNESS: u8 = 0;
pub const MAX_BRIGHTNESS: u8 = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Register {
    PwmPlus0ClockSource,
    PwmPlus0Period,
    PortBSel0,
    PwmPlus0Generator,
    PwmPlus0Config,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteKind {
    Or { bits: u32 },
    Clear { mask: u32 },
    Set { bits: u32 },
    Assign,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BacklightWrite {
    pub register: Register,
    pub kind: WriteKind,
    pub value: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BacklightSnapshot {
    pub pwm_plus0_clksrc: u32,
    pub portb_sel0: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BacklightInitPlan {
    writes: [BacklightWrite; 6],
}

impl BacklightInitPlan {
    pub const fn from_snapshot(snapshot: BacklightSnapshot) -> Self {
        let clock_source = BacklightWrite {
            register: Register::PwmPlus0ClockSource,
            kind: WriteKind::Or {
                bits: CLOCK_SOURCE_BITS,
            },
            value: snapshot.pwm_plus0_clksrc | CLOCK_SOURCE_BITS,
        };
        let period = BacklightWrite {
            register: Register::PwmPlus0Period,
            kind: WriteKind::Assign,
            value: 1023,
        };
        let portb_sel0_clear = BacklightWrite {
            register: Register::PortBSel0,
            kind: WriteKind::Clear {
                mask: PORTB_SEL0_B6_MASK,
            },
            value: snapshot.portb_sel0 & !PORTB_SEL0_B6_MASK,
        };
        let portb_sel0_set = BacklightWrite {
            register: Register::PortBSel0,
            kind: WriteKind::Set {
                bits: PORTB_SEL0_B6_PWMP0_CH0,
            },
            value: portb_sel0_clear.value | PORTB_SEL0_B6_PWMP0_CH0,
        };
        let generator = BacklightWrite {
            register: Register::PwmPlus0Generator,
            kind: WriteKind::Assign,
            value: PWM_PLUS0_GEN,
        };
        let config = BacklightWrite {
            register: Register::PwmPlus0Config,
            kind: WriteKind::Assign,
            value: PWM_PLUS0_CFG,
        };

        Self {
            writes: [
                clock_source,
                period,
                portb_sel0_clear,
                portb_sel0_set,
                generator,
                config,
            ],
        }
    }

    pub fn writes(&self) -> impl Iterator<Item = BacklightWrite> + '_ {
        self.writes.iter().copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BrightnessOutOfRange {
    pub brightness: u8,
}

pub const fn brightness_compare_value(brightness: u8) -> Result<u32, BrightnessOutOfRange> {
    if brightness > MAX_BRIGHTNESS {
        return Err(BrightnessOutOfRange { brightness });
    }

    Ok((1_u32 << brightness) - 1)
}
