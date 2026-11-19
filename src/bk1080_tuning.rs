//! Offline command plan for the BK1080 FM receiver, based on
//! [`driver/bk1080.c` at the pinned reference revision](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/bk1080.c#L103-L142).
//! Frequencies are expressed in 100 kHz units. This is a pure model: it does
//! not access a bus, write hardware, or sleep.
//!
//! The reference accepts every `u16` frequency without checking the selected
//! band's limits. Its channel subtraction wraps when assigned to `u16`; this
//! model preserves that behavior for parity, rather than providing safe
//! out-of-range rejection. Callers that need range validation must do so
//! separately.

#![cfg_attr(not(test), no_std)]

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BandLimits {
    pub low: u16,
    pub high: u16,
}

pub const fn band_limits(band: u8) -> BandLimits {
    match band % 4 {
        0 => BandLimits {
            low: 875,
            high: 1080,
        },
        1 => BandLimits {
            low: 760,
            high: 1080,
        },
        2 => BandLimits {
            low: 760,
            high: 900,
        },
        _ => BandLimits {
            low: 640,
            high: 760,
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Register {
    PowerConfiguration = 0x02,
    Channel = 0x03,
    SystemConfiguration2 = 0x05,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    WriteRegister { register: Register, value: u16 },
    WaitMs(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TunePlan {
    commands: [Command; 4],
}

impl TunePlan {
    pub const fn new(frequency: u16, band: u8, system_configuration2: u16) -> Self {
        let limits = band_limits(band);
        let channel = frequency.wrapping_sub(limits.low);
        let band_field = ((band & 0b11) as u16) << 6;
        let configuration = (system_configuration2 & !(0b11 << 6)) | band_field;

        Self {
            commands: [
                Command::WriteRegister {
                    register: Register::SystemConfiguration2,
                    value: configuration,
                },
                Command::WriteRegister {
                    register: Register::Channel,
                    value: channel,
                },
                Command::WaitMs(10),
                Command::WriteRegister {
                    register: Register::Channel,
                    value: channel | 0x8000,
                },
            ],
        }
    }

    pub fn commands(&self) -> impl Iterator<Item = Command> + '_ {
        self.commands.iter().copied()
    }
}

pub const fn mute_command(muted: bool) -> Command {
    Command::WriteRegister {
        register: Register::PowerConfiguration,
        value: if muted { 0x4201 } else { 0x0201 },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrequencyDeviation {
    pub base_frequency: u16,
    pub deviation: u16,
}

pub const fn frequency_deviation(base_frequency: u16, register_07: u16) -> FrequencyDeviation {
    FrequencyDeviation {
        base_frequency,
        deviation: register_07 / 16,
    }
}
