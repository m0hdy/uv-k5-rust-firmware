#[inline(never)]
#[unsafe(no_mangle)]
pub extern "C" fn mixed_rust_channel_valid(channel: u16) -> u8 {
    u8::from(channel <= 199)
}
