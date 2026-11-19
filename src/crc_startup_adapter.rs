#![cfg_attr(not(test), no_std)]

//! Pinned CRC_Init word writes for the DP32G030 CRC peripheral.
//!
//! The sequence follows `driver/crc.c` and `bsp/dp32g030/crc.h` at
//! [`7607f0a`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
//! The integrated board startup dispatcher uses this adapter conditionally;
//! the firmware entry point does not invoke that dispatcher.

pub trait WordIo {
    fn write_word(&mut self, address: usize, value: u32);
}

pub fn apply_with_io(io: &mut impl WordIo) {
    io.write_word(0x4000_3000, 0x0000_0100);
    io.write_word(0x4000_3004, 0);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct VolatileIo;

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl WordIo for VolatileIo {
    fn write_word(&mut self, address: usize, value: u32) {
        // SAFETY: The executor supplies aligned CRC addresses; the caller
        // guarantees their mapping and access.
        unsafe { core::ptr::write_volatile(address as *mut u32, value) }
    }
}

/// Applies the pinned CRC initialization to DP32G030 registers.
///
/// # Safety
///
/// The caller must ensure the DP32G030 CRC registers at `0x40003000` and
/// `0x40003004` are mapped and writable as aligned 32-bit words, with the CRC
/// clock enabled and appropriate privilege. The caller must own the entire CR
/// and IV registers and serialize the whole call against all other users,
/// including interrupt handlers. Both writes replace the complete register.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn apply_to_hardware() {
    apply_with_io(&mut VolatileIo);
}
