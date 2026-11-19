#![cfg_attr(not(test), no_std)]

#[path = "board_sequence.rs"]
#[allow(unused_attributes)]
mod board_sequence;
#[path = "gpio_init_plan.rs"]
#[allow(dead_code, unused_attributes)]
mod gpio_init_plan;
#[path = "portcon_input_plan.rs"]
#[allow(unused_attributes)]
mod portcon_input_plan;
#[path = "portcon_plan.rs"]
#[allow(unused_attributes)]
mod portcon_plan;

pub use board_sequence::{Call, Defines};
pub use gpio_init_plan::{
    Features as GpioFeatures, GpioPlan, GpioSnapshot, Port as GpioPort, Register as GpioRegister,
    Update as GpioUpdate,
};
pub use portcon_input_plan::PortconWrite as InputWrite;
pub use portcon_input_plan::{
    Features as PortconInputFeatures, PortconInputPlan, PortconInputSnapshot,
    Register as InputRegister, WriteKind as InputWriteKind,
};
pub use portcon_plan::PortconWrite as SelWrite;
pub use portcon_plan::{
    Features as PortconSelFeatures, PortconPlan, PortconSnapshot, Register as SelRegister,
    WriteKind as SelWriteKind,
};

const EVENT_CAPACITY: usize = 42;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Snapshots {
    pub portcon_sel: PortconSnapshot,
    pub portcon_input: PortconInputSnapshot,
    pub gpio: GpioSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Features {
    pub defines: Defines,
    pub enable_swd: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    Call(Call),
    PortconSel(SelWrite),
    PortconInput(InputWrite),
    Gpio(GpioUpdate),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Trace {
    events: [Event; EVENT_CAPACITY],
    len: usize,
    gpio_after: GpioSnapshot,
}

impl Trace {
    fn new(gpio: GpioSnapshot) -> Self {
        Self {
            events: [Event::Call(Call::DeviceClockGates); EVENT_CAPACITY],
            len: 0,
            gpio_after: gpio,
        }
    }

    fn push(&mut self, event: Event) {
        self.events[self.len] = event;
        self.len += 1;
    }

    pub fn events(&self) -> &[Event] {
        &self.events[..self.len]
    }

    pub const fn gpio_after(&self) -> GpioSnapshot {
        self.gpio_after
    }
}

struct Composer {
    trace: Trace,
    portcon_sel: PortconPlan,
    portcon_input: PortconInputPlan,
    gpio: GpioPlan,
}

impl board_sequence::Backend for Composer {
    fn call(&mut self, call: Call) {
        match call {
            Call::PortconInit => {
                for write in self.portcon_sel.writes() {
                    self.trace.push(Event::PortconSel(write));
                }
                for write in self.portcon_input.writes() {
                    self.trace.push(Event::PortconInput(write));
                }
            }
            Call::GpioInit => {
                for update in self.gpio.updates() {
                    self.trace.push(Event::Gpio(update));
                }
                self.gpio.apply(&mut self.trace.gpio_after);
            }
            other => self.trace.push(Event::Call(other)),
        }
    }
}

pub fn compose(snapshots: Snapshots, features: Features) -> Trace {
    let portcon_sel = PortconPlan::from_snapshot(
        snapshots.portcon_sel,
        portcon_plan::Features {
            enable_swd: features.enable_swd,
        },
    );
    let portcon_input = PortconInputPlan::from_snapshot(
        snapshots.portcon_input,
        portcon_input_plan::Features {
            enable_swd: features.enable_swd,
        },
    );
    let gpio = GpioPlan::for_features(gpio_init_plan::Features {
        enable_fmradio: features.defines.enable_fmradio,
    });
    let mut composer = Composer {
        trace: Trace::new(snapshots.gpio),
        portcon_sel,
        portcon_input,
        gpio,
    };

    board_sequence::run(features.defines, &mut composer);
    composer.trace
}
