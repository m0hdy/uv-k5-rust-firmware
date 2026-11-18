//! Pure memory-channel validity decisions based on the pinned C reference's
//! `RADIO_CheckValidChannel` in
//! [`radio.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/radio.c#L59-L80)
//! and channel bounds in
//! [`misc.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/misc.h#L39-L52).
//! The pinned revision and approved fixture provenance are documented in
//! [`reference-vector-provenance.md`](../docs/reference-vector-provenance.md).
//! Callers provide captured attributes and scan priorities; this API does not
//! access EEPROM or firmware globals.

const MEMORY_CHANNEL_LAST: u16 = 199;
const BAND_LAST: u8 = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChannelAttributes {
    pub band: u8,
    pub scan_list_1: bool,
    pub scan_list_2: bool,
}

pub fn is_valid_channel(
    channel: u16,
    attributes: ChannelAttributes,
    check_scan_list: bool,
    scan_list: u8,
    priority_channel_1: u8,
    priority_channel_2: u8,
) -> bool {
    if channel > MEMORY_CHANNEL_LAST || attributes.band > BAND_LAST {
        return false;
    }

    if !check_scan_list || scan_list > 1 {
        return true;
    }

    let is_member = if scan_list == 0 {
        attributes.scan_list_1
    } else {
        attributes.scan_list_2
    };
    is_member
        && channel != u16::from(priority_channel_1)
        && channel != u16::from(priority_channel_2)
}
