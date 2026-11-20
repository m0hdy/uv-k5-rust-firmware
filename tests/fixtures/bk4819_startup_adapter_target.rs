#![no_std]

#[path = "../../src/bk4819_startup_adapter.rs"]
mod startup;

unsafe extern "C" {
    fn caller_supplied_delay_1us();
}

struct CallerDelay;

impl startup::transport::Delay1us for CallerDelay {
    fn delay_1us(&mut self) {
        // SAFETY: This external symbol is a compile-time stand-in only.
        unsafe { caller_supplied_delay_1us() }
    }
}

pub unsafe fn instantiate_target_cold_init(gpio_output_shadow: &mut u16) {
    // SAFETY: This fixture is compiled only, never executed, with the documented mapping.
    unsafe { startup::initialize_mmio(&mut CallerDelay, gpio_output_shadow) }
}
