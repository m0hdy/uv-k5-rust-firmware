#![cfg_attr(not(test), no_std)]

//! Raw-data decoder for the VFO channel selections at EEPROM `0x0E80`.
//! This module does not perform EEPROM access or startup integration.

pub const EEPROM_OFFSET: u16 = 0x0E80;
pub const EEPROM_LEN: usize = 8;

const MR_CHANNEL_LAST: u8 = 199;
const FREQ_CHANNEL_FIRST: u8 = 200;
const FREQ_CHANNEL_LAST: u8 = 206;
const NOAA_CHANNEL_FIRST: u8 = 207;
const NOAA_CHANNEL_LAST: u8 = 216;
const LAST_CHANNEL: u8 = 217;
const BAND6_400_MHZ: u8 = 5;
const DEFAULT_VFO_FREQUENCY_CHANNEL: u8 = FREQ_CHANNEL_FIRST + BAND6_400_MHZ;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChannelSelectionBootDefaults {
    pub screen_channel: [u8; 2],
    pub mr_channel: [u8; 2],
    pub freq_channel: [u8; 2],
    pub noaa_channel: Option<[u8; 2]>,
}

const fn is_valid_channel(channel: u8) -> bool {
    channel < LAST_CHANNEL
}

const fn is_mr_channel(channel: u8) -> bool {
    channel <= MR_CHANNEL_LAST
}

const fn is_freq_channel(channel: u8) -> bool {
    channel >= FREQ_CHANNEL_FIRST && channel <= FREQ_CHANNEL_LAST
}

const fn is_noaa_channel(channel: u8) -> bool {
    channel >= NOAA_CHANNEL_FIRST && channel <= NOAA_CHANNEL_LAST
}

pub const fn decode_channel_selection_boot_defaults(
    data: [u8; EEPROM_LEN],
    noaa_enabled: bool,
) -> ChannelSelectionBootDefaults {
    ChannelSelectionBootDefaults {
        screen_channel: [
            if is_valid_channel(data[0]) {
                data[0]
            } else {
                DEFAULT_VFO_FREQUENCY_CHANNEL
            },
            if is_valid_channel(data[3]) {
                data[3]
            } else {
                DEFAULT_VFO_FREQUENCY_CHANNEL
            },
        ],
        mr_channel: [
            if is_mr_channel(data[1]) { data[1] } else { 0 },
            if is_mr_channel(data[4]) { data[4] } else { 0 },
        ],
        freq_channel: [
            if is_freq_channel(data[2]) {
                data[2]
            } else {
                DEFAULT_VFO_FREQUENCY_CHANNEL
            },
            if is_freq_channel(data[5]) {
                data[5]
            } else {
                DEFAULT_VFO_FREQUENCY_CHANNEL
            },
        ],
        noaa_channel: if noaa_enabled {
            Some([
                if is_noaa_channel(data[6]) {
                    data[6]
                } else {
                    NOAA_CHANNEL_FIRST
                },
                if is_noaa_channel(data[7]) {
                    data[7]
                } else {
                    NOAA_CHANNEL_FIRST
                },
            ])
        } else {
            None
        },
    }
}
