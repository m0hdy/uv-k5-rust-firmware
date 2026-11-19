#[allow(dead_code)]
#[path = "../src/backlight_startup_executor.rs"]
mod backlight_startup_executor;

use backlight_startup_executor::{Register, RegisterAddress};

#[test]
fn address_map_matches_the_pinned_dp32g030_registers() {
    let mappings = [
        (Register::PwmPlus0ClockSource, 0x400b_4008),
        (Register::PwmPlus0Period, 0x400b_401c),
        (Register::PortBSel0, 0x400b_0008),
        (Register::PwmPlus0Generator, 0x400b_4004),
        (Register::PwmPlus0Config, 0x400b_4000),
    ];

    for (register, expected_address) in mappings {
        let address = RegisterAddress::from_register(register).address();
        assert_eq!(address, expected_address);
        assert_eq!(address % core::mem::align_of::<u32>(), 0);
    }
}
