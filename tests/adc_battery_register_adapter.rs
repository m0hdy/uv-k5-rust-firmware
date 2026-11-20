pub use uv_k5_rust_firmware::startup_battery_sampling;

#[path = "../src/adc_battery_register_adapter.rs"]
mod adc_battery_register_adapter;

use adc_battery_register_adapter::{
    ReadRegister, ReadRegisterAddress, WriteRegister, WriteRegisterAddress, read_register_address,
    write_register_address,
};

#[test]
fn read_and_write_maps_match_pinned_aligned_register_addresses() {
    let read_addresses = [
        read_register_address(ReadRegister::Channel9Status).address(),
        read_register_address(ReadRegister::Channel4Data).address(),
        read_register_address(ReadRegister::Channel9Data).address(),
        read_register_address(ReadRegister::Start).address(),
    ];
    let write_addresses = [
        write_register_address(WriteRegister::Start).address(),
        write_register_address(WriteRegister::InterruptFlags).address(),
    ];

    assert_eq!(
        read_addresses,
        [0x400b_a058, 0x400b_a034, 0x400b_a05c, 0x400b_a004]
    );
    assert_eq!(write_addresses, [0x400b_a004, 0x400b_a00c]);
    assert!(
        read_addresses
            .into_iter()
            .chain(write_addresses)
            .all(|address| address.is_multiple_of(core::mem::align_of::<u32>()))
    );

    assert_eq!(
        read_register_address(ReadRegister::Channel9Status),
        ReadRegisterAddress::Channel9Status
    );
    assert_eq!(
        write_register_address(WriteRegister::InterruptFlags),
        WriteRegisterAddress::InterruptFlags
    );
}
