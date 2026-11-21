#![no_std]
#![no_main]

use core::{arch::global_asm, panic::PanicInfo};
use core::num::NonZeroU32;

#[path = "../../src/bk1080_startup_adapter.rs"]
#[allow(dead_code, unused_attributes, unused_imports)]
mod bk1080_startup_adapter;
#[path = "../../src/display_boot_defaults.rs"]
#[allow(dead_code, unused_attributes, unused_imports)]
mod display_boot_defaults;
#[path = "../../src/eeprom_i2c_bridge.rs"]
#[allow(dead_code, unused_attributes, unused_imports)]
mod eeprom_i2c_bridge;
#[path = "../../src/eeprom_read_transport.rs"]
#[allow(dead_code, unused_attributes, unused_imports)]
mod eeprom_read_transport;
#[path = "../../src/eeprom_transaction.rs"]
#[allow(dead_code, unused_attributes, unused_imports)]
mod eeprom_transaction;
#[path = "../../src/i2c_bitbang_flow.rs"]
#[allow(dead_code, unused_attributes, unused_imports)]
mod i2c_bitbang_flow;
#[path = "../../src/settings_boot_composition.rs"]
#[allow(dead_code, unused_attributes, unused_imports)]
mod settings_boot_composition;
#[path = "../../src/settings_boot_defaults.rs"]
#[allow(dead_code, unused_attributes, unused_imports)]
mod settings_boot_defaults;
#[path = "../../src/st7565_display.rs"]
#[allow(dead_code, unused_attributes, unused_imports)]
mod st7565_display;
#[path = "../../src/st7565_line_transport.rs"]
#[allow(dead_code, unused_attributes, unused_imports)]
mod st7565_line_transport;
#[path = "../../src/st7565_mmio_transport.rs"]
#[allow(dead_code, unused_attributes, unused_imports)]
mod st7565_mmio_transport;
#[path = "../../src/st7565_startup.rs"]
#[allow(dead_code, unused_attributes, unused_imports)]
mod st7565_startup;
#[path = "../../src/st7565_startup_executor.rs"]
#[allow(dead_code, unused_attributes, unused_imports)]
mod st7565_startup_executor;
#[path = "../../src/startup_version_display.rs"]
#[allow(dead_code, unused_attributes, unused_imports)]
mod startup_version_display;
#[path = "../../src/systick_timing.rs"]
#[allow(dead_code, unused_attributes, unused_imports)]
mod systick_timing;
#[path = "../../src/board_startup_adapters.rs"]
#[allow(unused_attributes)]
mod board_startup_adapters;

use board_startup_adapters::{Defines, run_hardware};
use i2c_bitbang_flow::target_gpio::Delay1us;
use st7565_startup_executor::target_spi0::DelayMs;

global_asm!(include_str!("startup.S"));

#[unsafe(no_mangle)]
pub extern "C" fn SysTick_Handler() {}

// Link-only forwarding input; this is not a production trigger selection.
const LINK_ONLY_EXTTRIG_SEL: u16 = 0x5aa5;

struct LinkOnlyDelay;

impl DelayMs for LinkOnlyDelay {
    fn delay_ms(&mut self, _milliseconds: u16) {}
}

impl Delay1us for LinkOnlyDelay {
    fn delay_1us(&mut self) {}
}

#[unsafe(no_mangle)]
pub extern "C" fn witness_main() -> ! {
    let mut delay = LinkOnlyDelay;
    let mut fm_delay = LinkOnlyDelay;
    // SAFETY: The link-only witness is not configured for hardware execution.
    let _ = unsafe {
        run_hardware(
            Defines {
                enable_fmradio: true,
                enable_uart: true,
                enabled_aircopy: false,
            },
            false,
            LINK_ONLY_EXTTRIG_SEL,
            &mut delay,
            &mut fm_delay,
            NonZeroU32::MIN,
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
