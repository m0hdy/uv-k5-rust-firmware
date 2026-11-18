#[path = "../src/channel_codec.rs"]
mod channel_codec;

use channel_codec::{
    CHANNEL_RECORD_LEN, ChannelRecord, CodecError, SETTINGS_BLOCK_OFFSETS, Settings, SettingsImage,
};

fn standard_settings() -> Settings {
    Settings {
        call_channel: 7,
        squelch_level: 5,
        tx_timeout_timer: 4,
        key_lock: false,
        vox_switch: false,
        vox_level: 3,
        mic_sensitivity: 2,
        backlight_min: 2,
        backlight_max: 8,
        channel_display_mode: 2,
        cross_band_rx_tx: 1,
        battery_save: 3,
        dual_watch: 1,
        backlight_time: 5,
        tail_tone_elimination: false,
        vfo_open: true,
        beep_control: false,
        key_m_long_press_action: 6,
        key_1_short_press_action: 3,
        key_1_long_press_action: 4,
        key_2_short_press_action: 5,
        key_2_long_press_action: 7,
        scan_resume_mode: 1,
        auto_keypad_lock: false,
        power_on_display_mode: 2,
        rssi_s0_level: 130,
        rssi_s9_level: 76,
        roger: 1,
        repeater_tail_tone_elimination: 5,
        tx_vfo: 0,
        battery_type: 2,
        dtmf_side_tone: false,
        dtmf_separate_code: b'*',
        dtmf_group_call_code: b'#',
        dtmf_decode_response: 1,
        dtmf_auto_reset_seconds: 10,
        dtmf_preload_ms: 300,
        dtmf_first_code_persist_ms: 500,
        dtmf_hash_code_persist_ms: 600,
        dtmf_code_persist_ms: 700,
        dtmf_code_interval_ms: 800,
        permit_remote_kill: false,
        scan_list_default: 1,
        scan_list_enabled: [true, false],
        scan_list_priority_ch1: [10, 12],
        scan_list_priority_ch2: [11, 13],
        tx_lock_mode: 0,
        tx_350_enabled: false,
        killed: false,
        tx_200_enabled: false,
        tx_500_enabled: false,
        tx_350_unlocked: false,
        scramble_enabled: false,
        live_dtmf_decoder: true,
        battery_text: 1,
        mic_bar: true,
        am_fix: false,
        backlight_on_tx_rx: 2,
    }
}

fn boundary_settings() -> Settings {
    let mut settings = standard_settings();
    settings.call_channel = 199;
    settings.squelch_level = 9;
    settings.tx_timeout_timer = 10;
    settings.key_lock = true;
    settings.vox_switch = true;
    settings.vox_level = 9;
    settings.mic_sensitivity = 4;
    settings.backlight_min = 9;
    settings.backlight_max = 10;
    settings.channel_display_mode = 3;
    settings.cross_band_rx_tx = 2;
    settings.battery_save = 4;
    settings.dual_watch = 2;
    settings.backlight_time = 9;
    settings.tail_tone_elimination = true;
    settings.vfo_open = false;
    settings.beep_control = true;
    settings.key_m_long_press_action = 14;
    settings.key_1_short_press_action = 14;
    settings.key_1_long_press_action = 14;
    settings.key_2_short_press_action = 14;
    settings.key_2_long_press_action = 14;
    settings.scan_resume_mode = 2;
    settings.auto_keypad_lock = true;
    settings.power_on_display_mode = 3;
    settings.rssi_s0_level = 150;
    settings.rssi_s9_level = 60;
    settings.roger = 2;
    settings.repeater_tail_tone_elimination = 10;
    settings.tx_vfo = 1;
    settings.battery_type = 4;
    settings.dtmf_side_tone = true;
    settings.dtmf_separate_code = b'A';
    settings.dtmf_group_call_code = b'D';
    settings.dtmf_decode_response = 3;
    settings.dtmf_auto_reset_seconds = 60;
    settings.dtmf_preload_ms = 1000;
    settings.dtmf_first_code_persist_ms = 1000;
    settings.dtmf_hash_code_persist_ms = 1000;
    settings.dtmf_code_persist_ms = 1000;
    settings.dtmf_code_interval_ms = 1000;
    settings.permit_remote_kill = true;
    settings.scan_list_default = 2;
    settings.scan_list_enabled = [false, true];
    settings.scan_list_priority_ch1 = [199, 199];
    settings.scan_list_priority_ch2 = [199, 199];
    settings.tx_lock_mode = 7;
    settings.tx_350_enabled = true;
    settings.tx_200_enabled = true;
    settings.tx_500_enabled = true;
    settings.tx_350_unlocked = true;
    settings.scramble_enabled = true;
    settings.live_dtmf_decoder = false;
    settings.battery_text = 2;
    settings.mic_bar = false;
    settings.am_fix = true;
    settings.backlight_on_tx_rx = 3;
    settings
}

#[test]
fn settings_profiles_match_pinned_c_eeprom_blocks_byte_for_byte() {
    let fixtures = [
        (
            standard_settings(),
            [
                [0x07, 0x05, 0x04, 0x00, 0x00, 0x00, 0x03, 0x02],
                [0x28, 0x02, 0x01, 0x03, 0x01, 0x05, 0x00, 0x01],
                [0x0c, 0x03, 0x04, 0x05, 0x07, 0x01, 0x00, 0x02],
                [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
                [0xff, 0x82, 0x4c, 0xff, 0xff, 0xff, 0xff, 0xff],
                [0x00, 0x01, 0x05, 0x00, 0x02, 0xff, 0xff, 0xff],
                [0x00, 0x2a, 0x23, 0x01, 0x0a, 0x1e, 0x32, 0x3c],
                [0x46, 0x50, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff],
                [0x01, 0x01, 0x0a, 0x0b, 0x00, 0x0c, 0x0d, 0xff],
                [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x97],
            ],
        ),
        (
            boundary_settings(),
            [
                [0xc7, 0x09, 0x0a, 0x00, 0x01, 0x01, 0x09, 0x04],
                [0x9a, 0x03, 0x02, 0x04, 0x02, 0x09, 0x01, 0x00],
                [0x1d, 0x0e, 0x0e, 0x0e, 0x0e, 0x02, 0x01, 0x03],
                [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
                [0xff, 0x96, 0x3c, 0xff, 0xff, 0xff, 0xff, 0xff],
                [0x00, 0x02, 0x0a, 0x01, 0x04, 0xff, 0xff, 0xff],
                [0x01, 0x41, 0x44, 0x03, 0x3c, 0x64, 0x64, 0x64],
                [0x64, 0x64, 0x01, 0xff, 0xff, 0xff, 0xff, 0xff],
                [0x02, 0x00, 0xc7, 0xc7, 0x01, 0xc7, 0xc7, 0xff],
                [0x07, 0x01, 0x00, 0x01, 0x01, 0x01, 0x01, 0xe9],
            ],
        ),
    ];

    assert_eq!(
        SETTINGS_BLOCK_OFFSETS,
        [
            0x0e70, 0x0e78, 0x0e90, 0x0e98, 0x0ea0, 0x0ea8, 0x0ed0, 0x0ed8, 0x0f18, 0x0f40,
        ]
    );
    for (settings, expected) in fixtures {
        let image = settings.encode().unwrap();
        assert_eq!(image.blocks, expected);
        assert_eq!(image.decode(), Ok(settings));
        for (offset, bytes) in SETTINGS_BLOCK_OFFSETS.iter().zip(expected.iter()) {
            assert_eq!(image.block(*offset), Some(bytes));
        }
    }
}

#[test]
fn channel_record_matches_pinned_c_eeprom_layout() {
    let channel = ChannelRecord {
        rx_frequency: 14_550_000,
        tx_offset: 600_000,
        rx_code: 23,
        tx_code: 42,
        rx_code_type: 2,
        tx_code_type: 3,
        tx_offset_direction: 2,
        modulation: 1,
        frequency_reverse: true,
        channel_bandwidth: true,
        output_power: 2,
        busy_channel_lock: 3,
        dtmf_decoding_enabled: true,
        dtmf_ptt_id_tx_mode: 4,
        step_setting: 6,
        scrambling_type: 5,
    };
    let expected = [
        0xf0, 0x03, 0xde, 0x00, 0xc0, 0x27, 0x09, 0x00, 0x17, 0x2a, 0x32, 0x12, 0x3b, 0x09, 0x06,
        0x05,
    ];

    assert_eq!(CHANNEL_RECORD_LEN, expected.len());
    assert_eq!(channel.encode(), Ok(expected));
    assert_eq!(ChannelRecord::decode(&expected), Ok(channel));
}

#[test]
fn channel_record_rejects_unrepresentable_fields_and_reserved_bits() {
    let mut channel = ChannelRecord {
        rx_frequency: 0,
        tx_offset: 0,
        rx_code: 0,
        tx_code: 0,
        rx_code_type: 16,
        tx_code_type: 0,
        tx_offset_direction: 0,
        modulation: 0,
        frequency_reverse: false,
        channel_bandwidth: false,
        output_power: 0,
        busy_channel_lock: 0,
        dtmf_decoding_enabled: false,
        dtmf_ptt_id_tx_mode: 0,
        step_setting: 0,
        scrambling_type: 0,
    };
    assert_eq!(
        channel.encode(),
        Err(CodecError::InvalidField("rx_code_type"))
    );

    channel.rx_code_type = 0;
    let mut bytes = channel.encode().unwrap();
    bytes[13] = 0x10;
    assert_eq!(
        ChannelRecord::decode(&bytes),
        Err(CodecError::InvalidField("channel DTMF reserved bits"))
    );
}

#[test]
fn settings_codec_rejects_corrupt_and_unrepresentable_values() {
    let mut image = standard_settings().encode().unwrap();
    image.blocks[0][4] = 2;
    assert_eq!(image.decode(), Err(CodecError::InvalidField("key_lock")));

    let mut image = standard_settings().encode().unwrap();
    image.blocks[3][0] = 0;
    assert_eq!(
        image.decode(),
        Err(CodecError::InvalidPadding {
            offset: 0x0e98,
            index: 0
        })
    );

    let mut invalid = standard_settings();
    invalid.squelch_level = 10;
    assert_eq!(
        invalid.encode(),
        Err(CodecError::InvalidField("squelch_level"))
    );

    let mut invalid = standard_settings();
    invalid.dtmf_preload_ms = 301;
    assert_eq!(
        invalid.encode(),
        Err(CodecError::InvalidField("dtmf_preload_ms"))
    );
}

#[test]
fn settings_image_has_exactly_the_supported_write_blocks() {
    assert_eq!(core::mem::size_of::<SettingsImage>(), 80);
    assert_eq!(SETTINGS_BLOCK_OFFSETS.len() * 8, 80);
}
