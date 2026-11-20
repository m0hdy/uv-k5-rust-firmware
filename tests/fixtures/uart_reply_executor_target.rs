#![no_std]

#[allow(dead_code, unused_attributes)]
#[path = "../../src/uart_reply_executor.rs"]
mod executor;

pub unsafe fn instantiate_target_reply(payload: &mut [u8]) -> Result<(), executor::ReplyError> {
    // SAFETY: This fixture is compiled only and never executed.
    unsafe { executor::send_reply_mmio(payload, true, core::num::NonZeroU32::new(1).unwrap()) }
}
