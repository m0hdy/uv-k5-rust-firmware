#![cfg_attr(not(test), no_std)]

//! Startup defaults for the core settings EEPROM block at `0x0E70`.
//! This is an isolated decoder; it does not perform EEPROM access.

pub const EEPROM_OFFSET: u16 = 0x0E70;
pub const EEPROM_LEN: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeaturePresence {
    pub noaa_enabled: bool,
    pub vox_enabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreSettings {
    pub call_channel: u8,
    pub squelch_level: u8,
    pub tx_timeout_timer: u8,
    pub noaa_auto_scan: Option<bool>,
    pub key_lock: bool,
    pub vox_switch: Option<bool>,
    pub vox_level: Option<u8>,
    pub mic_sensitivity: u8,
}

pub const fn decode_core_settings(
    data: [u8; EEPROM_LEN],
    features: FeaturePresence,
) -> CoreSettings {
    CoreSettings {
        call_channel: if data[0] <= 199 { data[0] } else { 0 },
        squelch_level: if data[1] < 10 { data[1] } else { 1 },
        tx_timeout_timer: if data[2] < 11 { data[2] } else { 1 },
        noaa_auto_scan: if features.noaa_enabled {
            Some(data[3] == 1)
        } else {
            None
        },
        key_lock: data[4] == 1,
        vox_switch: if features.vox_enabled {
            Some(data[5] == 1)
        } else {
            None
        },
        vox_level: if features.vox_enabled {
            Some(if data[6] < 10 { data[6] } else { 1 })
        } else {
            None
        },
        mic_sensitivity: if data[7] < 5 { data[7] } else { 4 },
    }
}
