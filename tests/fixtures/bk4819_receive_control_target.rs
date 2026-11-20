#![no_std]

#[allow(dead_code)]
#[path = "../../src/bk4819_receive_control_adapter.rs"]
mod adapter;

unsafe extern "C" {
    fn caller_supplied_delay_1us();
}

struct CallerDelay;

impl adapter::Delay1us for CallerDelay {
    fn delay_1us(&mut self) {
        // SAFETY: This external symbol is a compile-time stand-in only.
        unsafe { caller_supplied_delay_1us() }
    }
}

pub unsafe fn instantiate_target_receive_on() {
    // SAFETY: This fixture is compiled only and never executed.
    unsafe { adapter::receive_on_mmio(&mut CallerDelay) }
}

pub unsafe fn instantiate_target_sleep() {
    // SAFETY: This fixture is compiled only and never executed.
    unsafe { adapter::sleep_mmio(&mut CallerDelay) }
}

pub unsafe fn instantiate_target_idle() {
    // SAFETY: This fixture is compiled only and never executed.
    unsafe { adapter::idle_mmio(&mut CallerDelay) }
}
