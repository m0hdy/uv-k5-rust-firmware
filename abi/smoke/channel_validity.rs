#![no_std]

#[path = "../../src/channel_validity.rs"]
mod channel_validity;

/// # Safety
///
/// `out_valid` must be non-null, properly aligned, and point to a live,
/// writable `u8` for the duration of this call. The caller must prevent
/// conflicting accesses while the function writes to that storage.
///
/// A null output pointer or any scan-list flag greater than `1` is rejected
/// before writing. Other scalar arguments are passed to the channel-validity
/// routine without further validation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uvk5_channel_validity(
    channel: u16,
    band: u8,
    scan_list_1: u8,
    scan_list_2: u8,
    check_scan_list: u8,
    scan_list: u8,
    priority_channel_1: u8,
    priority_channel_2: u8,
    out_valid: *mut u8,
) -> i32 {
    if out_valid.is_null()
        || scan_list_1 > 1
        || scan_list_2 > 1
        || check_scan_list > 1
    {
        return 1;
    }

    let valid = channel_validity::is_valid_channel(
        channel,
        channel_validity::ChannelAttributes {
            band,
            scan_list_1: scan_list_1 != 0,
            scan_list_2: scan_list_2 != 0,
        },
        check_scan_list != 0,
        scan_list,
        priority_channel_1,
        priority_channel_2,
    );
    unsafe {
        out_valid.write(u8::from(valid));
    }
    0
}
