#![cfg_attr(target_arch = "arm", no_std)]
#![cfg_attr(target_arch = "arm", no_main)]

#[cfg(target_arch = "arm")]
use core::arch::global_asm;
#[cfg(target_arch = "arm")]
use core::panic::PanicInfo;

#[cfg(target_arch = "arm")]
global_asm!(include_str!("startup.S"));

#[cfg(all(target_arch = "arm", feature = "firmware-integration"))]
#[path = "firmware/firmware_integration.rs"]
mod firmware_integration;

#[cfg(target_arch = "arm")]
#[unsafe(no_mangle)]
pub extern "C" fn firmware_main() -> ! {
    #[cfg(feature = "firmware-integration")]
    {
        firmware_integration::firmware_integration_entry()
    }
    #[cfg(not(feature = "firmware-integration"))]
    {
        loop {
            core::hint::spin_loop();
        }
    }
}

#[cfg(target_arch = "arm")]
#[unsafe(no_mangle)]
#[inline(never)]
pub extern "C" fn SysTick_Handler() {
    #[cfg(feature = "firmware-integration")]
    firmware_integration::systick_interrupt();
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
