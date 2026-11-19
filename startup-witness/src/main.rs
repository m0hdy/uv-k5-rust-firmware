#![no_std]
#![no_main]

use core::{arch::global_asm, panic::PanicInfo};

#[path = "../../src/board_startup_adapters.rs"]
#[allow(unused_attributes)]
mod board_startup_adapters;

use board_startup_adapters::{Defines, run_hardware};

global_asm!(include_str!("startup.S"));

#[unsafe(no_mangle)]
pub extern "C" fn witness_main() -> ! {
    // SAFETY: The link-only witness is not configured for hardware execution.
    let _ = unsafe {
        run_hardware(
            Defines {
                enable_fmradio: false,
                enable_uart: false,
                enabled_aircopy: false,
            },
            false,
        )
    };
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
