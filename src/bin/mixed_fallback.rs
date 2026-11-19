#![no_std]
#![no_main]

use core::arch::global_asm;
use core::panic::PanicInfo;

#[path = "../../mixed-fallback/rust_impl.rs"]
mod rust_impl;

global_asm!(include_str!("startup.S"));

#[cfg(feature = "mixed-fallback-c")]
unsafe extern "C" {
    fn mixed_c_channel_valid(channel: u16) -> u8;
}

static INPUT_CHANNEL: u16 = 200;
static mut RESULT: u8 = 0;

#[cfg(feature = "mixed-fallback-c")]
fn selected_channel_valid(channel: u16) -> u8 {
    unsafe { mixed_c_channel_valid(channel) }
}

#[cfg(feature = "mixed-fallback-rust")]
fn selected_channel_valid(channel: u16) -> u8 {
    rust_impl::mixed_rust_channel_valid(channel)
}

#[unsafe(no_mangle)]
pub extern "C" fn firmware_main() -> ! {
    let channel = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(INPUT_CHANNEL)) };
    unsafe {
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(RESULT),
            selected_channel_valid(channel),
        );
    }
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
