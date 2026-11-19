#![cfg_attr(not(test), no_std)]

//! Startup defaults for the display settings EEPROM block at `0x0E78`.
//! This is an isolated decoder; it does not perform EEPROM access.

pub const EEPROM_OFFSET: u16 = 0x0E78;
pub const EEPROM_LEN: usize = 8;
pub const BACKLIGHT_SUBMENU_LEN: usize = 8;

const MDF_FREQUENCY: u8 = 0;
const CROSS_BAND_OFF: u8 = 0;
const DUAL_WATCH_CHAN_A: u8 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeaturePresence {
    pub blmin_tmp_off_enabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BacklightMinStatus {
    On,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisplayBootDefaults {
    pub backlight_min: u8,
    pub backlight_max: u8,
    pub backlight_min_status: Option<BacklightMinStatus>,
    pub channel_display_mode: u8,
    pub cross_band_rx_tx: u8,
    pub battery_save: u8,
    pub dual_watch: u8,
    pub backlight_time: u8,
    pub tail_tone_elimination: bool,
    pub vfo_open: bool,
}

pub const fn decode_display_boot_defaults(
    data: [u8; EEPROM_LEN],
    features: FeaturePresence,
) -> DisplayBootDefaults {
    let max_nibble = data[0] & 0x0f;
    let backlight_max = if max_nibble <= 10 { max_nibble } else { 10 };
    let min_nibble = data[0] >> 4;

    DisplayBootDefaults {
        backlight_min: if min_nibble < backlight_max {
            min_nibble
        } else {
            0
        },
        backlight_max,
        backlight_min_status: if features.blmin_tmp_off_enabled {
            Some(BacklightMinStatus::On)
        } else {
            None
        },
        channel_display_mode: if data[1] < 4 { data[1] } else { MDF_FREQUENCY },
        cross_band_rx_tx: if data[2] < 3 { data[2] } else { CROSS_BAND_OFF },
        battery_save: if data[3] < 5 { data[3] } else { 4 },
        dual_watch: if data[4] < 3 {
            data[4]
        } else {
            DUAL_WATCH_CHAN_A
        },
        backlight_time: if data[5] < BACKLIGHT_SUBMENU_LEN as u8 {
            data[5]
        } else {
            3
        },
        tail_tone_elimination: data[6] == 1,
        vfo_open: data[7] != 0,
    }
}
