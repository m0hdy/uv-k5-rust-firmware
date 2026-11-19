#![no_std]

#[path = "../../src/bk4819_register_transport.rs"]
mod transport;

unsafe extern "C" {
    fn caller_supplied_delay_1us();
}

struct CallerDelay;

impl transport::Delay1us for CallerDelay {
    fn delay_1us(&mut self) {
        // SAFETY: This external symbol is a compile-time stand-in only.
        unsafe { caller_supplied_delay_1us() }
    }
}

pub unsafe fn instantiate_target_read() -> u16 {
    // SAFETY: This fixture is compiled only; it is never executed.
    unsafe { transport::read_register_mmio(&mut CallerDelay, 0x26) }
}

pub unsafe fn instantiate_target_write() {
    // SAFETY: This fixture is compiled only; it is never executed.
    unsafe { transport::write_register_mmio(&mut CallerDelay, 0x35, 0x96a1) }
}
