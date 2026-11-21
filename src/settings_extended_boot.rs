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
pub const CHANNEL_ATTRIBUTES_OFFSET: u16 = 0x0d60;
pub const CHANNEL_ATTRIBUTES_LEN: usize = 207;
pub const DTMF_ANI_OFFSET: u16 = 0x0ee0;
pub const DTMF_KILL_OFFSET: u16 = 0x0ee8;
pub const DTMF_REVIVE_OFFSET: u16 = 0x0ef0;
pub const DTMF_UP_OFFSET: u16 = 0x0ef8;
pub const DTMF_DOWN_OFFSET: u16 = 0x0f08;
pub const FM_CONFIG_OFFSET: u16 = 0x0e88;
pub const FM_CHANNELS_OFFSET: u16 = 0x0e40;
pub const FM_CHANNEL_COUNT: usize = 20;
pub const BATTERY_TYPE_UNKNOWN: u8 = 2;
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

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistedChannelAttributes(u8);

impl PersistedChannelAttributes {
    pub const fn band(self) -> u8 {
        self.0 & 0x0f
    }

    pub const fn compander(self) -> u8 {
        (self.0 >> 4) & 0x03
    }

    pub const fn scan_list_2(self) -> bool {
        self.0 & 0x40 != 0
    }

    pub const fn scan_list_1(self) -> bool {
        self.0 & 0x80 != 0
    }
}

pub const fn decode_channel_attributes(
    bytes: [u8; CHANNEL_ATTRIBUTES_LEN],
) -> [PersistedChannelAttributes; CHANNEL_ATTRIBUTES_LEN] {
    let mut decoded = [PersistedChannelAttributes(0); CHANNEL_ATTRIBUTES_LEN];
    let mut index = 0;
    while index < CHANNEL_ATTRIBUTES_LEN {
        let value = bytes[index];
        decoded[index] = if value == 0xff {
            PersistedChannelAttributes(0x0f)
        } else {
            PersistedChannelAttributes(value)
        };
        index += 1;
    }
    decoded
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DtmfCode<const LEN: usize> {
    /// Bytes after an EEPROM terminator are retained, as in the C buffer.
    pub bytes: [u8; LEN],
    pub used_fallback: bool,
}

pub const fn decode_dtmf_code<const LEN: usize>(
    mut data: [u8; LEN],
    fallback: [u8; LEN],
) -> DtmfCode<LEN> {
    if LEN == 0 || data[0] == 0xff || data[0] == 0 {
        return DtmfCode {
            bytes: fallback,
            used_fallback: true,
        };
    }
    let mut index = 0;
    while index < LEN {
        let value = data[index];
        if value == 0xff || value == 0 {
            data[index] = 0;
            return DtmfCode {
                bytes: data,
                used_fallback: false,
            };
        }
        if !valid_dtmf(value) {
            return DtmfCode {
                bytes: fallback,
                used_fallback: true,
            };
        }
        index += 1;
    }
    DtmfCode {
        bytes: data,
        used_fallback: false,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DtmfCodeSettings {
    pub ani: Option<DtmfCode<8>>,
    pub kill: Option<DtmfCode<8>>,
    pub revive: Option<DtmfCode<8>>,
    pub up: DtmfCode<16>,
    pub down: DtmfCode<16>,
}

pub const fn decode_dtmf_codes(
    calling: Option<[[u8; 8]; 3]>,
    up: [u8; 16],
    down: [u8; 16],
) -> DtmfCodeSettings {
    let (ani, kill, revive) = match calling {
        Some(codes) => (
            Some(decode_dtmf_code(codes[0], *b"123\0\0\0\0\0")),
            Some(decode_dtmf_code(codes[1], *b"ABCD9\0\0\0")),
            Some(decode_dtmf_code(codes[2], *b"9DCBA\0\0\0")),
        ),
        None => (None, None, None),
    };
    DtmfCodeSettings {
        ani,
        kill,
        revive,
        up: decode_dtmf_code(up, *b"12345\0\0\0\0\0\0\0\0\0\0\0"),
        down: decode_dtmf_code(down, *b"54321\0\0\0\0\0\0\0\0\0\0\0"),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FmBootSettings {
    pub band: u8,
    pub selected_frequency: u16,
    pub selected_channel: u8,
    pub memory_mode: bool,
    pub playing_frequency: u16,
    pub channels: [u16; FM_CHANNEL_COUNT],
}

pub const fn decode_fm_boot(config: [u8; 4], channel_bytes: [u8; 40]) -> FmBootSettings {
    let selected_raw = u16::from_le_bytes([config[0], config[1]]);
    let selected_channel = config[2];
    let memory_mode = config[3] & 1 != 0;
    let band = (config[3] >> 1) & 0x03;
    let limits = crate::bk1080_receive_adapter::band_limits(band);
    let selected_frequency = if selected_raw >= limits.low && selected_raw <= limits.high {
        selected_raw
    } else {
        limits.low
    };

    let mut channels = [0; FM_CHANNEL_COUNT];
    let mut index = 0;
    while index < FM_CHANNEL_COUNT {
        channels[index] =
            u16::from_le_bytes([channel_bytes[index * 2], channel_bytes[index * 2 + 1]]);
        index += 1;
    }

    let mut result = FmBootSettings {
        band,
        selected_frequency,
        selected_channel,
        memory_mode,
        playing_frequency: selected_frequency,
        channels,
    };
    if memory_mode {
        let mut candidate = selected_channel;
        let mut attempt = 0;
        let mut found = false;
        while attempt < FM_CHANNEL_COUNT {
            if candidate == 0xff {
                candidate = (FM_CHANNEL_COUNT - 1) as u8;
            } else if candidate >= FM_CHANNEL_COUNT as u8 {
                candidate = 0;
            }
            let frequency = channels[candidate as usize];
            if frequency >= limits.low && frequency < limits.high {
                result.selected_channel = candidate;
                result.playing_frequency = frequency;
                found = true;
                break;
            }
            candidate = candidate.wrapping_add(1);
            attempt += 1;
        }
        if !found {
            result.memory_mode = false;
        }
    }
    result
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

pub const fn decode_roger_and_transmitter(data: [u8; 8]) -> RogerAndTransmitterSettings {
    RogerAndTransmitterSettings {
        roger: if data[1] < 3 { data[1] } else { 0 },
        repeater_tail_tone_elimination: if data[2] < 11 { data[2] } else { 0 },
        tx_vfo: if data[3] < 2 { data[3] } else { 0 },
        battery_type: if data[4] < BATTERY_TYPE_UNKNOWN {
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
    pub fm_enabled: bool,
    pub voice_enabled: bool,
    pub rssi_bar_enabled: bool,
    pub alarm_enabled: bool,
    pub dtmf_calling_enabled: bool,
    pub audio_bar_enabled: bool,
    pub am_fix_enabled: bool,
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
        roger_and_transmitter: decode_roger_and_transmitter(blocks[3]),
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
