#![cfg_attr(not(test), no_std)]

#[allow(dead_code, unused_attributes)]
#[path = "backlight_plan.rs"]
mod backlight_plan;

use backlight_plan::{BacklightInitPlan, BacklightSnapshot};
pub use backlight_plan::{Register, WriteKind};

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
