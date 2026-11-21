#![cfg_attr(not(test), no_std)]

//! Portable decoding for the standard `SETTINGS_InitEEPROM` blocks after
//! channel selection. EEPROM transport and optional device calibration remain
//! outside this module.

pub const ACTION_COUNT: u8 = 15;
pub const ACTION_NONE: u8 = 0;
pub const ACTION_MONITOR: u8 = 3;
pub const ACTION_SCAN: u8 = 4;
pub const CUSTOM_AES_KEY_OFFSET: u16 = 0x0f30;
pub const CUSTOM_AES_KEY_LEN: usize = 16;
pub const EEPROM_OFFSETS: [u16; 8] = [
    0x0e90, 0x0e98, 0x0ea0, 0x0ea8, 0x0ed0, 0x0ed8, 0x0f18, 0x0f40,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CustomAesKey {
    pub bytes: [u8; CUSTOM_AES_KEY_LEN],
    pub present: bool,
}

pub const fn decode_custom_aes_key(bytes: [u8; CUSTOM_AES_KEY_LEN]) -> CustomAesKey {
    let mut present = false;
    let mut index = 0;
    while index < CUSTOM_AES_KEY_LEN {
        if bytes[index] != 0xff {
            present = true;
        }
        index += 1;
    }
    CustomAesKey { bytes, present }
}

pub const fn decode_power_on_password(data: [u8; 8]) -> [u8; 4] {
    [data[0], data[1], data[2], data[3]]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VoiceAndRssiSettings {
    pub voice_prompt: Option<u8>,
    pub rssi_calibration: Option<(u8, u8)>,
}

pub const fn decode_voice_and_rssi(
    data: [u8; 8],
    voice_enabled: bool,
    rssi_bar_enabled: bool,
) -> VoiceAndRssiSettings {
    let s0 = data[1];
    let s9 = data[2];
    let valid_rssi = s0 > 90 && s0 < 160 && s9 > 50 && s9 < s0.saturating_sub(9);
    VoiceAndRssiSettings {
        voice_prompt: if voice_enabled {
            Some(if data[0] < 3 { data[0] } else { 2 })
        } else {
            None
        },
        rssi_calibration: if rssi_bar_enabled {
            Some(if valid_rssi { (s0, s9) } else { (130, 76) })
        } else {
            None
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AlarmSetting {
    pub alarm_mode: Option<u8>,
}

pub const fn decode_alarm(data: [u8; 8], alarm_enabled: bool) -> AlarmSetting {
    AlarmSetting {
        alarm_mode: if alarm_enabled {
            Some(if data[0] < 2 { data[0] } else { 1 })
        } else {
            None
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyAndScanSettings {
    pub beep_control: bool,
    pub key_m_long_press_action: u8,
    pub key_1_short_press_action: u8,
    pub key_1_long_press_action: u8,
    pub key_2_short_press_action: u8,
    pub key_2_long_press_action: u8,
    pub scan_resume_mode: u8,
    pub auto_keypad_lock: bool,
    pub power_on_display_mode: u8,
}

pub const fn decode_key_and_scan(data: [u8; 8]) -> KeyAndScanSettings {
    KeyAndScanSettings {
        beep_control: data[0] & 1 != 0,
        key_m_long_press_action: if data[0] >> 1 < ACTION_COUNT {
            data[0] >> 1
        } else {
            ACTION_NONE
        },
        key_1_short_press_action: if data[1] < ACTION_COUNT {
            data[1]
        } else {
            ACTION_MONITOR
        },
        key_1_long_press_action: if data[2] < ACTION_COUNT {
            data[2]
        } else {
            ACTION_NONE
        },
        key_2_short_press_action: if data[3] < ACTION_COUNT {
            data[3]
        } else {
            ACTION_SCAN
        },
        key_2_long_press_action: if data[4] < ACTION_COUNT {
            data[4]
        } else {
            ACTION_NONE
        },
        // SCAN_RESUME_CO is the second enum variant (value 1).
        scan_resume_mode: if data[5] < 3 { data[5] } else { 1 },
        auto_keypad_lock: data[6] == 1,
        power_on_display_mode: if data[7] < 4 { data[7] } else { 2 },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RogerAndTransmitterSettings {
    pub roger: u8,
    pub repeater_tail_tone_elimination: u8,
    pub tx_vfo: u8,
    pub battery_type: u8,
}

pub const fn decode_roger_and_transmitter(
    data: [u8; 8],
    battery_type_count: u8,
) -> RogerAndTransmitterSettings {
    RogerAndTransmitterSettings {
        roger: if data[1] < 3 { data[1] } else { 0 },
        repeater_tail_tone_elimination: if data[2] < 11 { data[2] } else { 0 },
        tx_vfo: if data[3] < 2 { data[3] } else { 0 },
        battery_type: if data[4] < battery_type_count {
            data[4]
        } else {
            0
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DtmfSettings {
    pub side_tone: bool,
    pub separate_code: Option<u8>,
    pub group_call_code: Option<u8>,
    pub decode_response: Option<u8>,
    pub auto_reset_seconds: Option<u8>,
    pub preload_ms: u16,
    pub first_code_persist_ms: u16,
    pub hash_code_persist_ms: u16,
    pub code_persist_ms: u16,
    pub code_interval_ms: u16,
    pub permit_remote_kill: Option<bool>,
}

pub const fn decode_dtmf(
    first: [u8; 8],
    second: [u8; 8],
    dtmf_calling_enabled: bool,
) -> DtmfSettings {
    DtmfSettings {
        side_tone: if first[0] < 2 { first[0] == 1 } else { true },
        separate_code: if dtmf_calling_enabled {
            Some(if valid_dtmf(first[1]) { first[1] } else { b'*' })
        } else {
            None
        },
        group_call_code: if dtmf_calling_enabled {
            Some(if valid_dtmf(first[2]) { first[2] } else { b'#' })
        } else {
            None
        },
        decode_response: if dtmf_calling_enabled {
            Some(if first[3] < 4 { first[3] } else { 0 })
        } else {
            None
        },
        auto_reset_seconds: if dtmf_calling_enabled {
            Some(if first[4] >= 5 && first[4] < 61 {
                first[4]
            } else {
                10
            })
        } else {
            None
        },
        preload_ms: tenths_or(first[5], 300),
        first_code_persist_ms: tenths_or(first[6], 100),
        hash_code_persist_ms: tenths_or(first[7], 100),
        code_persist_ms: tenths_or(second[0], 100),
        code_interval_ms: tenths_or(second[1], 100),
        permit_remote_kill: if dtmf_calling_enabled {
            Some(if second[2] < 2 { second[2] == 1 } else { true })
        } else {
            None
        },
    }
}

const fn tenths_or(value: u8, fallback: u16) -> u16 {
    if value < 101 {
        (value as u16) * 10
    } else {
        fallback
    }
}

const fn valid_dtmf(value: u8) -> bool {
    (value >= b'0' && value <= b'9')
        || (value >= b'A' && value <= b'D')
        || value == b'*'
        || value == b'#'
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanLists {
    pub default: u8,
    pub enabled: [bool; 2],
    pub priority_ch1: [u8; 2],
    pub priority_ch2: [u8; 2],
}

pub const fn decode_scan_lists(data: [u8; 8]) -> ScanLists {
    ScanLists {
        default: if data[0] < 3 { data[0] } else { 0 },
        enabled: [data[1] < 2 && data[1] == 1, data[4] < 2 && data[4] == 1],
        priority_ch1: [data[2], data[5]],
        priority_ch2: [data[3], data[6]],
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GlobalSettings {
    pub tx_lock_mode: u8,
    pub tx_350_enabled: bool,
    pub killed: Option<bool>,
    pub tx_200_enabled: bool,
    pub tx_500_enabled: bool,
    pub tx_350_unlocked: bool,
    pub scramble_enabled: bool,
    pub live_dtmf_decoder: bool,
    pub battery_text: u8,
    pub mic_bar: Option<bool>,
    pub am_fix: Option<bool>,
    pub backlight_on_tx_rx: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GlobalFeatures {
    pub dtmf_calling_enabled: bool,
    pub audio_bar_enabled: bool,
    pub am_fix_enabled: bool,
}

pub const fn decode_global(
    data: [u8; 8],
    lock_modes: u8,
    features: GlobalFeatures,
) -> GlobalSettings {
    GlobalSettings {
        tx_lock_mode: if data[0] < lock_modes { data[0] } else { 0 },
        tx_350_enabled: data[1] == 1,
        killed: if features.dtmf_calling_enabled {
            Some(data[2] == 1)
        } else {
            None
        },
        tx_200_enabled: data[3] == 1,
        tx_500_enabled: data[4] == 1,
        tx_350_unlocked: if data[5] < 2 { data[5] == 1 } else { true },
        scramble_enabled: if data[6] < 2 { data[6] == 1 } else { true },
        live_dtmf_decoder: data[7] & 2 != 0,
        battery_text: {
            let value = (data[7] >> 2) & 3;
            if value <= 2 { value } else { 2 }
        },
        mic_bar: if features.audio_bar_enabled {
            Some(data[7] & 0x10 != 0)
        } else {
            None
        },
        am_fix: if features.am_fix_enabled {
            Some(data[7] & 0x20 != 0)
        } else {
            None
        },
        backlight_on_tx_rx: data[7] >> 6,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExtendedBootFeatures {
    pub voice_enabled: bool,
    pub rssi_bar_enabled: bool,
    pub alarm_enabled: bool,
    pub dtmf_calling_enabled: bool,
    pub audio_bar_enabled: bool,
    pub am_fix_enabled: bool,
    pub battery_type_count: u8,
    pub lock_mode_count: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExtendedBootSettings {
    pub keys_and_scan: KeyAndScanSettings,
    pub power_on_password: [u8; 4],
    pub voice_and_rssi: VoiceAndRssiSettings,
    pub alarm: AlarmSetting,
    pub roger_and_transmitter: RogerAndTransmitterSettings,
    pub dtmf: DtmfSettings,
    pub scan_lists: ScanLists,
    pub global: GlobalSettings,
}

/// Decodes the eight standard settings blocks as one portable startup snapshot.
/// Block order is given by [`EEPROM_OFFSETS`]; feature-presence inputs are
/// explicit because the pinned C build conditionally omits several fields.
pub const fn decode_extended_boot(
    blocks: [[u8; 8]; EEPROM_OFFSETS.len()],
    features: ExtendedBootFeatures,
) -> ExtendedBootSettings {
    ExtendedBootSettings {
        keys_and_scan: decode_key_and_scan(blocks[0]),
        power_on_password: decode_power_on_password(blocks[1]),
        voice_and_rssi: decode_voice_and_rssi(
            blocks[2],
            features.voice_enabled,
            features.rssi_bar_enabled,
        ),
        alarm: decode_alarm(blocks[3], features.alarm_enabled),
        roger_and_transmitter: decode_roger_and_transmitter(blocks[3], features.battery_type_count),
        dtmf: decode_dtmf(blocks[4], blocks[5], features.dtmf_calling_enabled),
        scan_lists: decode_scan_lists(blocks[6]),
        global: decode_global(
            blocks[7],
            features.lock_mode_count,
            GlobalFeatures {
                dtmf_calling_enabled: features.dtmf_calling_enabled,
                audio_bar_enabled: features.audio_bar_enabled,
                am_fix_enabled: features.am_fix_enabled,
            },
        ),
    }
}
