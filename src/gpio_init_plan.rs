//! Pure offline model of `BOARD_GPIO_Init` at pinned reference commit
//! `7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`
//! ([`board.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/board.c)
//! and [GPIO pin definitions](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/gpio.h)).
//! This is an independently authored command plan, not copied C, MMIO access,
//! port-mux setup, or evidence of on-device parity.
//!
//! The pinned default enables `ENABLE_FMRADIO`, so the last update sets B15
//! (BK1080 data) high. With that feature disabled, the update is absent; the
//! routine does not initialize any other output-data bits. `ENABLE_SWD` only
//! changes the separate port-mux routine and is outside this model.
//!
//! Direction bits A7/A8 are left untouched because the source assumes their
//! bootloader-provided UART defaults; B14 is also left untouched, with the
//! source assuming it is an input. These assumptions and the pin assignments
//! have not been reconciled against board revisions or verified on hardware.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Port {
    A,
    B,
    C,
}

impl Port {
    const fn index(self) -> usize {
        match self {
            Self::A => 0,
            Self::B => 1,
            Self::C => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Register {
    Direction,
    Data,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    SetBits,
    ClearBits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Update {
    pub port: Port,
    pub register: Register,
    pub operation: Operation,
    pub mask: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Features {
    pub enable_fmradio: bool,
}

pub const PINNED_DEFAULT_FEATURES: Features = Features {
    enable_fmradio: true,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpioSnapshot {
    pub direction: [u32; 3],
    pub data: [u32; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpioPlan {
    direction_updates: [Update; 5],
    fmradio_data_update: Option<Update>,
}

impl GpioPlan {
    pub const fn for_features(features: Features) -> Self {
        Self {
            direction_updates: [
                Update {
                    port: Port::A,
                    register: Register::Direction,
                    operation: Operation::SetBits,
                    mask: (1 << 10) | (1 << 11) | (1 << 12) | (1 << 13),
                },
                Update {
                    port: Port::A,
                    register: Register::Direction,
                    operation: Operation::ClearBits,
                    mask: (1 << 3) | (1 << 4) | (1 << 5) | (1 << 6),
                },
                Update {
                    port: Port::B,
                    register: Register::Direction,
                    operation: Operation::SetBits,
                    mask: (1 << 9) | (1 << 11) | (1 << 15),
                },
                Update {
                    port: Port::C,
                    register: Register::Direction,
                    operation: Operation::SetBits,
                    mask: (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 4),
                },
                Update {
                    port: Port::C,
                    register: Register::Direction,
                    operation: Operation::ClearBits,
                    mask: 1 << 5,
                },
            ],
            fmradio_data_update: if features.enable_fmradio {
                Some(Update {
                    port: Port::B,
                    register: Register::Data,
                    operation: Operation::SetBits,
                    mask: 1 << 15,
                })
            } else {
                None
            },
        }
    }

    pub fn updates(&self) -> impl Iterator<Item = Update> + '_ {
        self.direction_updates
            .iter()
            .copied()
            .chain(self.fmradio_data_update)
    }

    pub fn apply(&self, snapshot: &mut GpioSnapshot) {
        for update in self.updates() {
            let port = update.port.index();
            let register = match update.register {
                Register::Direction => &mut snapshot.direction[port],
                Register::Data => &mut snapshot.data[port],
            };
            match update.operation {
                Operation::SetBits => *register |= update.mask,
                Operation::ClearBits => *register &= !update.mask,
            }
        }
    }
}
