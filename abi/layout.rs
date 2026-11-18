#![no_std]

use core::mem::{align_of, offset_of, size_of};

#[repr(C)]
struct Uvk5LayoutWitness {
    status: i32,
    mode: u8,
    reserved: [u8; 3],
    frequency_hz: u32,
}

const _: () = {
    assert!(size_of::<Uvk5LayoutWitness>() == 12);
    assert!(align_of::<Uvk5LayoutWitness>() == 4);
    assert!(offset_of!(Uvk5LayoutWitness, status) == 0);
    assert!(offset_of!(Uvk5LayoutWitness, mode) == 4);
    assert!(offset_of!(Uvk5LayoutWitness, reserved) == 5);
    assert!(offset_of!(Uvk5LayoutWitness, frequency_hz) == 8);
};
