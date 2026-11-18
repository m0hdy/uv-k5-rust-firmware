//! Host-testable EEPROM codecs for the pinned channel and settings write formats.
//!
//! Channel records cover the 16 bytes written by `SETTINGS_SaveChannel`. Settings
//! cover only the ten 8-byte blocks captured for the pinned feature configuration.
//! Channel-record placement, unrelated channel attributes/names, optional-feature
//! data, EEPROM loading/default policy, and runtime frequency/DCS calculations are
//! out of scope. Neither format has a checksum; decoding can reject invalid/reserved
//! encodings but cannot detect arbitrary corruption that remains a valid encoding.

pub const CHANNEL_RECORD_LEN: usize = 16;
pub const SETTINGS_BLOCK_LEN: usize = 8;
pub const SETTINGS_BLOCK_OFFSETS: [u16; 10] = [
    0x0e70, 0x0e78, 0x0e90, 0x0e98, 0x0ea0, 0x0ea8, 0x0ed0, 0x0ed8, 0x0f18, 0x0f40,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecError {
    InvalidField(&'static str),
    InvalidPadding { offset: u16, index: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChannelRecord {
    pub rx_frequency: u32,
    pub tx_offset: u32,
    pub rx_code: u8,
    pub tx_code: u8,
    pub rx_code_type: u8,
    pub tx_code_type: u8,
    pub tx_offset_direction: u8,
    pub modulation: u8,
    pub frequency_reverse: bool,
    pub channel_bandwidth: bool,
    pub output_power: u8,
    pub busy_channel_lock: u8,
    pub dtmf_decoding_enabled: bool,
    pub dtmf_ptt_id_tx_mode: u8,
    pub step_setting: u8,
    pub scrambling_type: u8,
}

impl ChannelRecord {
    pub fn encode(self) -> Result<[u8; CHANNEL_RECORD_LEN], CodecError> {
        check("rx_code_type", self.rx_code_type <= 0x0f)?;
        check("tx_code_type", self.tx_code_type <= 0x0f)?;
        check("tx_offset_direction", self.tx_offset_direction <= 0x0f)?;
        check("modulation", self.modulation <= 0x0f)?;
        check("output_power", self.output_power <= 0x03)?;
        check("busy_channel_lock", self.busy_channel_lock <= 0x0f)?;
        check("dtmf_ptt_id_tx_mode", self.dtmf_ptt_id_tx_mode <= 0x07)?;

        let mut bytes = [0; CHANNEL_RECORD_LEN];
        bytes[0..4].copy_from_slice(&self.rx_frequency.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.tx_offset.to_le_bytes());
        bytes[8] = self.rx_code;
        bytes[9] = self.tx_code;
        bytes[10] = (self.tx_code_type << 4) | self.rx_code_type;
        bytes[11] = (self.modulation << 4) | self.tx_offset_direction;
        bytes[12] = (self.busy_channel_lock << 4)
            | (self.output_power << 2)
            | (u8::from(self.channel_bandwidth) << 1)
            | u8::from(self.frequency_reverse);
        bytes[13] = (self.dtmf_ptt_id_tx_mode << 1) | u8::from(self.dtmf_decoding_enabled);
        bytes[14] = self.step_setting;
        bytes[15] = self.scrambling_type;
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8; CHANNEL_RECORD_LEN]) -> Result<Self, CodecError> {
        check("channel DTMF reserved bits", bytes[13] & 0xf0 == 0)?;
        Ok(Self {
            rx_frequency: u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            tx_offset: u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            rx_code: bytes[8],
            tx_code: bytes[9],
            rx_code_type: bytes[10] & 0x0f,
            tx_code_type: bytes[10] >> 4,
            tx_offset_direction: bytes[11] & 0x0f,
            modulation: bytes[11] >> 4,
            frequency_reverse: bytes[12] & 1 != 0,
            channel_bandwidth: bytes[12] & 2 != 0,
            output_power: (bytes[12] >> 2) & 3,
            busy_channel_lock: bytes[12] >> 4,
            dtmf_decoding_enabled: bytes[13] & 1 != 0,
            dtmf_ptt_id_tx_mode: (bytes[13] >> 1) & 7,
            step_setting: bytes[14],
            scrambling_type: bytes[15],
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Settings {
    pub call_channel: u8,
    pub squelch_level: u8,
    pub tx_timeout_timer: u8,
    pub key_lock: bool,
    pub vox_switch: bool,
    pub vox_level: u8,
    pub mic_sensitivity: u8,
    pub backlight_min: u8,
    pub backlight_max: u8,
    pub channel_display_mode: u8,
    pub cross_band_rx_tx: u8,
    pub battery_save: u8,
    pub dual_watch: u8,
    pub backlight_time: u8,
    pub tail_tone_elimination: bool,
    pub vfo_open: bool,
    pub beep_control: bool,
    pub key_m_long_press_action: u8,
    pub key_1_short_press_action: u8,
    pub key_1_long_press_action: u8,
    pub key_2_short_press_action: u8,
    pub key_2_long_press_action: u8,
    pub scan_resume_mode: u8,
    pub auto_keypad_lock: bool,
    pub power_on_display_mode: u8,
    pub rssi_s0_level: u8,
    pub rssi_s9_level: u8,
    pub roger: u8,
    pub repeater_tail_tone_elimination: u8,
    pub tx_vfo: u8,
    pub battery_type: u8,
    pub dtmf_side_tone: bool,
    pub dtmf_separate_code: u8,
    pub dtmf_group_call_code: u8,
    pub dtmf_decode_response: u8,
    pub dtmf_auto_reset_seconds: u8,
    pub dtmf_preload_ms: u16,
    pub dtmf_first_code_persist_ms: u16,
    pub dtmf_hash_code_persist_ms: u16,
    pub dtmf_code_persist_ms: u16,
    pub dtmf_code_interval_ms: u16,
    pub permit_remote_kill: bool,
    pub scan_list_default: u8,
    pub scan_list_enabled: [bool; 2],
    pub scan_list_priority_ch1: [u8; 2],
    pub scan_list_priority_ch2: [u8; 2],
    pub tx_lock_mode: u8,
    pub tx_350_enabled: bool,
    pub killed: bool,
    pub tx_200_enabled: bool,
    pub tx_500_enabled: bool,
    pub tx_350_unlocked: bool,
    pub scramble_enabled: bool,
    pub live_dtmf_decoder: bool,
    pub battery_text: u8,
    pub mic_bar: bool,
    pub am_fix: bool,
    pub backlight_on_tx_rx: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettingsImage {
    pub blocks: [[u8; SETTINGS_BLOCK_LEN]; SETTINGS_BLOCK_OFFSETS.len()],
}

impl Settings {
    pub fn encode(self) -> Result<SettingsImage, CodecError> {
        self.validate()?;

        let mut blocks = [[0; SETTINGS_BLOCK_LEN]; SETTINGS_BLOCK_OFFSETS.len()];
        blocks[0] = [
            self.call_channel,
            self.squelch_level,
            self.tx_timeout_timer,
            0,
            u8::from(self.key_lock),
            u8::from(self.vox_switch),
            self.vox_level,
            self.mic_sensitivity,
        ];
        blocks[1] = [
            (self.backlight_min << 4) | self.backlight_max,
            self.channel_display_mode,
            self.cross_band_rx_tx,
            self.battery_save,
            self.dual_watch,
            self.backlight_time,
            u8::from(self.tail_tone_elimination),
            u8::from(self.vfo_open),
        ];
        blocks[2] = [
            u8::from(self.beep_control) | (self.key_m_long_press_action << 1),
            self.key_1_short_press_action,
            self.key_1_long_press_action,
            self.key_2_short_press_action,
            self.key_2_long_press_action,
            self.scan_resume_mode,
            u8::from(self.auto_keypad_lock),
            self.power_on_display_mode,
        ];
        blocks[3] = [0xff; SETTINGS_BLOCK_LEN];
        blocks[4] = [
            0xff,
            self.rssi_s0_level,
            self.rssi_s9_level,
            0xff,
            0xff,
            0xff,
            0xff,
            0xff,
        ];
        blocks[5] = [
            0,
            self.roger,
            self.repeater_tail_tone_elimination,
            self.tx_vfo,
            self.battery_type,
            0xff,
            0xff,
            0xff,
        ];
        blocks[6] = [
            u8::from(self.dtmf_side_tone),
            self.dtmf_separate_code,
            self.dtmf_group_call_code,
            self.dtmf_decode_response,
            self.dtmf_auto_reset_seconds,
            tenths(self.dtmf_preload_ms),
            tenths(self.dtmf_first_code_persist_ms),
            tenths(self.dtmf_hash_code_persist_ms),
        ];
        blocks[7] = [
            tenths(self.dtmf_code_persist_ms),
            tenths(self.dtmf_code_interval_ms),
            u8::from(self.permit_remote_kill),
            0xff,
            0xff,
            0xff,
            0xff,
            0xff,
        ];
        blocks[8] = [
            self.scan_list_default,
            u8::from(self.scan_list_enabled[0]),
            self.scan_list_priority_ch1[0],
            self.scan_list_priority_ch2[0],
            u8::from(self.scan_list_enabled[1]),
            self.scan_list_priority_ch1[1],
            self.scan_list_priority_ch2[1],
            0xff,
        ];

        let mut flags = 0xff;
        if !self.live_dtmf_decoder {
            flags &= !(1 << 1);
        }
        flags = (flags & !(3 << 2)) | (self.battery_text << 2);
        if !self.mic_bar {
            flags &= !(1 << 4);
        }
        if !self.am_fix {
            flags &= !(1 << 5);
        }
        flags = (flags & !(3 << 6)) | (self.backlight_on_tx_rx << 6);
        blocks[9] = [
            self.tx_lock_mode,
            u8::from(self.tx_350_enabled),
            u8::from(self.killed),
            u8::from(self.tx_200_enabled),
            u8::from(self.tx_500_enabled),
            u8::from(self.tx_350_unlocked),
            u8::from(self.scramble_enabled),
            flags,
        ];

        Ok(SettingsImage { blocks })
    }

    fn validate(self) -> Result<(), CodecError> {
        check("call_channel", self.call_channel <= 199)?;
        check("squelch_level", self.squelch_level <= 9)?;
        check("tx_timeout_timer", self.tx_timeout_timer <= 10)?;
        check("vox_level", self.vox_level <= 9)?;
        check("mic_sensitivity", self.mic_sensitivity <= 4)?;
        check("backlight_min", self.backlight_min <= 15)?;
        check("backlight_max", self.backlight_max <= 15)?;
        check("channel_display_mode", self.channel_display_mode <= 3)?;
        check("cross_band_rx_tx", self.cross_band_rx_tx <= 2)?;
        check("battery_save", self.battery_save <= 4)?;
        check("dual_watch", self.dual_watch <= 2)?;
        check("backlight_time", self.backlight_time <= 9)?;
        check(
            "key_m_long_press_action",
            self.key_m_long_press_action <= 14,
        )?;
        check(
            "key_1_short_press_action",
            self.key_1_short_press_action <= 14,
        )?;
        check(
            "key_1_long_press_action",
            self.key_1_long_press_action <= 14,
        )?;
        check(
            "key_2_short_press_action",
            self.key_2_short_press_action <= 14,
        )?;
        check(
            "key_2_long_press_action",
            self.key_2_long_press_action <= 14,
        )?;
        check("scan_resume_mode", self.scan_resume_mode <= 2)?;
        check("power_on_display_mode", self.power_on_display_mode <= 3)?;
        check(
            "RSSI calibration",
            self.rssi_s0_level > 90
                && self.rssi_s0_level < 160
                && self.rssi_s9_level > 50
                && self.rssi_s9_level < self.rssi_s0_level.saturating_sub(9),
        )?;
        check("roger", self.roger <= 2)?;
        check(
            "repeater_tail_tone_elimination",
            self.repeater_tail_tone_elimination <= 10,
        )?;
        check("tx_vfo", self.tx_vfo <= 1)?;
        check("dtmf_separate_code", is_dtmf_code(self.dtmf_separate_code))?;
        check(
            "dtmf_group_call_code",
            is_dtmf_code(self.dtmf_group_call_code),
        )?;
        check("dtmf_decode_response", self.dtmf_decode_response <= 3)?;
        check(
            "dtmf_auto_reset_seconds",
            (5..=60).contains(&self.dtmf_auto_reset_seconds),
        )?;
        check(
            "dtmf_preload_ms",
            self.dtmf_preload_ms <= 1000 && self.dtmf_preload_ms.is_multiple_of(10),
        )?;
        check(
            "dtmf_first_code_persist_ms",
            self.dtmf_first_code_persist_ms <= 1000
                && self.dtmf_first_code_persist_ms.is_multiple_of(10),
        )?;
        check(
            "dtmf_hash_code_persist_ms",
            self.dtmf_hash_code_persist_ms <= 1000
                && self.dtmf_hash_code_persist_ms.is_multiple_of(10),
        )?;
        check(
            "dtmf_code_persist_ms",
            self.dtmf_code_persist_ms <= 1000 && self.dtmf_code_persist_ms.is_multiple_of(10),
        )?;
        check(
            "dtmf_code_interval_ms",
            self.dtmf_code_interval_ms <= 1000 && self.dtmf_code_interval_ms.is_multiple_of(10),
        )?;
        check("scan_list_default", self.scan_list_default <= 2)?;
        check("tx_lock_mode", self.tx_lock_mode <= 7)?;
        check("battery_text", self.battery_text <= 2)?;
        check("backlight_on_tx_rx", self.backlight_on_tx_rx <= 3)?;
        Ok(())
    }
}

impl SettingsImage {
    pub fn block(&self, offset: u16) -> Option<&[u8; SETTINGS_BLOCK_LEN]> {
        SETTINGS_BLOCK_OFFSETS
            .iter()
            .position(|candidate| *candidate == offset)
            .map(|index| &self.blocks[index])
    }

    pub fn decode(&self) -> Result<Settings, CodecError> {
        let b = &self.blocks;
        check_bool("key_lock", b[0][4])?;
        check_bool("vox_switch", b[0][5])?;
        check_bool("tail_tone_elimination", b[1][6])?;
        check_bool("vfo_open", b[1][7])?;
        check_bool("beep_control", b[2][0] & 1)?;
        check_bool("auto_keypad_lock", b[2][6])?;
        check_bool("dtmf_side_tone", b[6][0])?;
        check_bool("permit_remote_kill", b[7][2])?;
        check_bool("scan_list_enabled[0]", b[8][1])?;
        check_bool("scan_list_enabled[1]", b[8][4])?;
        for (index, value) in b[9][1..7].iter().copied().enumerate() {
            check_bool(
                match index {
                    0 => "tx_350_enabled",
                    1 => "killed",
                    2 => "tx_200_enabled",
                    3 => "tx_500_enabled",
                    4 => "tx_350_unlocked",
                    _ => "scramble_enabled",
                },
                value,
            )?;
        }

        let settings = Settings {
            call_channel: b[0][0],
            squelch_level: b[0][1],
            tx_timeout_timer: b[0][2],
            key_lock: b[0][4] != 0,
            vox_switch: b[0][5] != 0,
            vox_level: b[0][6],
            mic_sensitivity: b[0][7],
            backlight_min: b[1][0] >> 4,
            backlight_max: b[1][0] & 0x0f,
            channel_display_mode: b[1][1],
            cross_band_rx_tx: b[1][2],
            battery_save: b[1][3],
            dual_watch: b[1][4],
            backlight_time: b[1][5],
            tail_tone_elimination: b[1][6] != 0,
            vfo_open: b[1][7] != 0,
            beep_control: b[2][0] & 1 != 0,
            key_m_long_press_action: b[2][0] >> 1,
            key_1_short_press_action: b[2][1],
            key_1_long_press_action: b[2][2],
            key_2_short_press_action: b[2][3],
            key_2_long_press_action: b[2][4],
            scan_resume_mode: b[2][5],
            auto_keypad_lock: b[2][6] != 0,
            power_on_display_mode: b[2][7],
            rssi_s0_level: b[4][1],
            rssi_s9_level: b[4][2],
            roger: b[5][1],
            repeater_tail_tone_elimination: b[5][2],
            tx_vfo: b[5][3],
            battery_type: b[5][4],
            dtmf_side_tone: b[6][0] != 0,
            dtmf_separate_code: b[6][1],
            dtmf_group_call_code: b[6][2],
            dtmf_decode_response: b[6][3],
            dtmf_auto_reset_seconds: b[6][4],
            dtmf_preload_ms: u16::from(b[6][5]) * 10,
            dtmf_first_code_persist_ms: u16::from(b[6][6]) * 10,
            dtmf_hash_code_persist_ms: u16::from(b[6][7]) * 10,
            dtmf_code_persist_ms: u16::from(b[7][0]) * 10,
            dtmf_code_interval_ms: u16::from(b[7][1]) * 10,
            permit_remote_kill: b[7][2] != 0,
            scan_list_default: b[8][0],
            scan_list_enabled: [b[8][1] != 0, b[8][4] != 0],
            scan_list_priority_ch1: [b[8][2], b[8][5]],
            scan_list_priority_ch2: [b[8][3], b[8][6]],
            tx_lock_mode: b[9][0],
            tx_350_enabled: b[9][1] != 0,
            killed: b[9][2] != 0,
            tx_200_enabled: b[9][3] != 0,
            tx_500_enabled: b[9][4] != 0,
            tx_350_unlocked: b[9][5] != 0,
            scramble_enabled: b[9][6] != 0,
            live_dtmf_decoder: b[9][7] & (1 << 1) != 0,
            battery_text: (b[9][7] >> 2) & 3,
            mic_bar: b[9][7] & (1 << 4) != 0,
            am_fix: b[9][7] & (1 << 5) != 0,
            backlight_on_tx_rx: b[9][7] >> 6,
        };
        let encoded = settings.encode()?;
        if encoded != *self {
            let (offset, index) = first_mismatch(&encoded, self);
            return Err(CodecError::InvalidPadding { offset, index });
        }
        Ok(settings)
    }
}

fn tenths(milliseconds: u16) -> u8 {
    (milliseconds / 10) as u8
}

fn is_dtmf_code(code: u8) -> bool {
    code.is_ascii_digit() || (b'A'..=b'D').contains(&code) || matches!(code, b'*' | b'#')
}

fn check(field: &'static str, valid: bool) -> Result<(), CodecError> {
    if valid {
        Ok(())
    } else {
        Err(CodecError::InvalidField(field))
    }
}

fn check_bool(field: &'static str, value: u8) -> Result<(), CodecError> {
    check(field, value <= 1)
}

fn first_mismatch(expected: &SettingsImage, actual: &SettingsImage) -> (u16, usize) {
    for (block_index, (expected, actual)) in
        expected.blocks.iter().zip(actual.blocks.iter()).enumerate()
    {
        if let Some(byte_index) = expected
            .iter()
            .zip(actual.iter())
            .position(|(expected, actual)| expected != actual)
        {
            return (SETTINGS_BLOCK_OFFSETS[block_index], byte_index);
        }
    }
    (SETTINGS_BLOCK_OFFSETS[0], 0)
}
