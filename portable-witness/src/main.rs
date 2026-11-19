#![cfg_attr(target_arch = "arm", no_std)]
#![cfg_attr(target_arch = "arm", no_main)]

#[cfg(target_arch = "arm")]
use core::{arch::global_asm, panic::PanicInfo};

#[cfg(target_arch = "arm")]
global_asm!(include_str!("startup.S"));

#[cfg(target_arch = "arm")]
#[unsafe(no_mangle)]
static mut WITNESS_RESULT: u32 = 0;

#[cfg(target_arch = "arm")]
#[unsafe(no_mangle)]
pub extern "C" fn witness_main() -> ! {
    let result = portable_size_witness::exercise();
    unsafe {
        core::ptr::write_volatile(&raw mut WITNESS_RESULT, result);
    }
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(target_arch = "arm")]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(not(target_arch = "arm"))]
fn main() {}
