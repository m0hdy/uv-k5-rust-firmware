use uv_k5_rust_firmware::settings_extended_boot::{
    EEPROM_OFFSETS, ExtendedBootFeatures, GlobalFeatures, decode_alarm, decode_channel_attributes,
    decode_custom_aes_key, decode_dtmf, decode_dtmf_code, decode_dtmf_codes, decode_extended_boot,
    decode_fm_boot, decode_global, decode_key_and_scan, decode_power_on_password,
    decode_roger_and_transmitter, decode_scan_lists, decode_voice_and_rssi,
};

#[test]
fn key_actions_keep_packed_c_width_and_apply_each_invalid_value_fallback() {
    let result = decode_key_and_scan([0xff, 15, 15, 15, 15, 3, 2, 4]);
    assert_eq!(result.key_m_long_press_action, 0);
    assert!(result.beep_control);
    assert_eq!(result.key_1_short_press_action, 3);
    assert_eq!(result.key_1_long_press_action, 0);
    assert_eq!(result.key_2_short_press_action, 4);
    assert_eq!(result.key_2_long_press_action, 0);
    assert_eq!(result.scan_resume_mode, 1);
    assert!(!result.auto_keypad_lock);
    assert_eq!(result.power_on_display_mode, 2);
}

#[test]
fn dtmf_tenths_conversion_and_fallbacks_match_pinned_startup() {
    let result = decode_dtmf(
        [2, b'X', b'#', 4, 61, 100, 101, 0],
        [100, 101, 2, 0, 0, 0, 0, 0],
        true,
    );
    assert!(result.side_tone);
    assert_eq!(result.separate_code, Some(b'*'));
    assert_eq!(result.group_call_code, Some(b'#'));
    assert_eq!(result.decode_response, Some(0));
    assert_eq!(result.auto_reset_seconds, Some(10));
    assert_eq!(result.preload_ms, 1000);
    assert_eq!(result.first_code_persist_ms, 100);
    assert_eq!(result.hash_code_persist_ms, 0);
    assert_eq!(result.code_persist_ms, 1000);
    assert_eq!(result.code_interval_ms, 100);
    assert_eq!(result.permit_remote_kill, Some(true));
    let disabled = decode_dtmf([0; 8], [0; 8], false);
    assert_eq!(disabled.separate_code, None);
    assert_eq!(disabled.permit_remote_kill, None);
}

#[test]
fn scan_list_enable_is_boolean_only_for_zero_or_one_and_priorities_are_raw_bytes() {
    assert_eq!(
        decode_scan_lists([3, 2, 0xff, 0xfe, 1, 7, 8, 0]),
        decode_scan_lists([0, 0, 0xff, 0xfe, 1, 7, 8, 0])
    );
    let result = decode_scan_lists([3, 2, 0xff, 0xfe, 1, 7, 8, 0]);
    assert_eq!(result.default, 0);
    assert_eq!(result.enabled, [false, true]);
    assert_eq!(result.priority_ch1, [0xff, 7]);
    assert_eq!(result.priority_ch2, [0xfe, 8]);
}

#[test]
fn global_and_transmitter_settings_use_pinned_defaults() {
    let global = decode_global(
        [9, 2, 1, 2, 1, 2, 0xff, 0xfc],
        7,
        GlobalFeatures {
            dtmf_calling_enabled: false,
            audio_bar_enabled: false,
            am_fix_enabled: false,
        },
    );
    assert_eq!(global.tx_lock_mode, 0);
    assert!(!global.tx_350_enabled);
    assert_eq!(global.killed, None);
    assert!(!global.tx_200_enabled);
    assert!(global.tx_500_enabled);
    assert!(global.tx_350_unlocked);
    assert!(global.scramble_enabled);
    assert_eq!(global.battery_text, 2);
    assert_eq!(global.backlight_on_tx_rx, 3);

    assert_eq!(
        decode_roger_and_transmitter([0, 3, 11, 2, 8, 0, 0, 0]).roger,
        0
    );
    assert_eq!(
        decode_roger_and_transmitter([0, 3, 11, 2, 1, 0, 0, 0]).battery_type,
        1
    );
    assert_eq!(
        decode_roger_and_transmitter([0, 3, 11, 2, 2, 0, 0, 0]).battery_type,
        0
    );
}

#[test]
fn password_voice_rssi_and_alarm_fields_preserve_feature_gates_and_defaults() {
    assert_eq!(
        decode_power_on_password([1, 2, 3, 4, 5, 6, 7, 8]),
        [1, 2, 3, 4]
    );
    assert_eq!(
        decode_voice_and_rssi([9, 130, 76, 0, 0, 0, 0, 0], true, true),
        decode_voice_and_rssi([2, 130, 76, 0, 0, 0, 0, 0], true, true)
    );
    assert_eq!(
        decode_voice_and_rssi([9, 130, 76, 0, 0, 0, 0, 0], true, true).voice_prompt,
        Some(2)
    );
    assert_eq!(
        decode_voice_and_rssi([0, 250, 250, 0, 0, 0, 0, 0], false, true).rssi_calibration,
        Some((130, 76))
    );
    let disabled_features = decode_voice_and_rssi([0; 8], false, false);
    assert_eq!(disabled_features.voice_prompt, None);
    assert_eq!(disabled_features.rssi_calibration, None);
    assert_eq!(
        decode_alarm([2, 0, 0, 0, 0, 0, 0, 0], true).alarm_mode,
        Some(1)
    );
    assert_eq!(decode_alarm([0; 8], false).alarm_mode, None);
}

#[test]
fn standard_blocks_compose_in_declared_eeprom_order() {
    assert_eq!(EEPROM_OFFSETS[0], 0x0e90);
    assert_eq!(EEPROM_OFFSETS[7], 0x0f40);
    let mut blocks = [[0; 8]; EEPROM_OFFSETS.len()];
    blocks[0] = [1, 2, 3, 4, 5, 2, 1, 3];
    blocks[1] = [9, 8, 7, 6, 5, 4, 3, 2];
    blocks[2] = [2, 130, 76, 0, 0, 0, 0, 0];
    blocks[3] = [1, 2, 3, 1, 2, 0, 0, 0];
    blocks[4] = [1, b'*', b'#', 2, 10, 30, 40, 50];
    blocks[5] = [60, 70, 1, 0, 0, 0, 0, 0];
    blocks[6] = [2, 1, 10, 11, 0, 12, 13, 0];
    blocks[7] = [8, 1, 1, 1, 1, 1, 1, 0xff];

    let decoded = decode_extended_boot(
        blocks,
        ExtendedBootFeatures {
            fm_enabled: false,
            voice_enabled: true,
            rssi_bar_enabled: true,
            alarm_enabled: true,
            dtmf_calling_enabled: true,
            audio_bar_enabled: true,
            am_fix_enabled: true,
            lock_mode_count: 8,
        },
    );
    assert_eq!(decoded.keys_and_scan.scan_resume_mode, 2);
    assert_eq!(decoded.power_on_password, [9, 8, 7, 6]);
    assert_eq!(decoded.voice_and_rssi.rssi_calibration, Some((130, 76)));
    assert_eq!(decoded.roger_and_transmitter.roger, 2);
    assert_eq!(decoded.dtmf.preload_ms, 300);
    assert_eq!(decoded.scan_lists.priority_ch2, [11, 13]);
    assert_eq!(decoded.global.tx_lock_mode, 0);
    assert_eq!(decoded.global.battery_text, 2);
}

#[test]
fn custom_aes_key_presence_matches_the_erased_word_check() {
    assert!(!decode_custom_aes_key([0xff; 16]).present);
    let mut key = [0xff; 16];
    key[7] = 0;
    assert!(decode_custom_aes_key(key).present);
}

#[test]
fn channel_attributes_preserve_bitfields_and_normalize_erased_entries() {
    let mut bytes = [0; 207];
    bytes[0] = 0xff;
    bytes[1] = 0xe3;
    let decoded = decode_channel_attributes(bytes);
    assert_eq!(decoded[0].band(), 0x0f);
    assert_eq!(decoded[0].compander(), 0);
    assert!(!decoded[0].scan_list_1());
    assert!(!decoded[0].scan_list_2());
    assert_eq!(decoded[1].band(), 3);
    assert_eq!(decoded[1].compander(), 2);
    assert!(decoded[1].scan_list_1());
    assert!(decoded[1].scan_list_2());
    assert_eq!(core::mem::size_of_val(&decoded), 207);
}

#[test]
fn dtmf_code_validation_matches_fixed_length_c_termination_and_fallbacks() {
    let accepted = decode_dtmf_code([b'1', b'2', b'A', 0xff, b'Z', 0, 0, 0], *b"123\0\0\0\0\0");
    assert_eq!(accepted.bytes, [b'1', b'2', b'A', 0, b'Z', 0, 0, 0]);
    assert!(!accepted.used_fallback);

    let invalid = decode_dtmf_code(*b"ABZ\0\0\0\0\0", *b"123\0\0\0\0\0");
    assert_eq!(invalid.bytes, *b"123\0\0\0\0\0");
    assert!(invalid.used_fallback);
    let empty = decode_dtmf_code([0xff; 8], *b"123\0\0\0\0\0");
    assert_eq!(empty.bytes, *b"123\0\0\0\0\0");

    let settings = decode_dtmf_codes(None, [0xff; 16], [0xff; 16]);
    assert_eq!(settings.ani, None);
    assert_eq!(settings.up.bytes, *b"12345\0\0\0\0\0\0\0\0\0\0\0");
    assert_eq!(settings.down.bytes, *b"54321\0\0\0\0\0\0\0\0\0\0\0");
}

#[test]
fn fm_boot_checks_selected_frequency_and_applies_memory_channel_fallback() {
    let mut channel_bytes = [0xff; 40];
    channel_bytes[0] = 900_u16 as u8;
    channel_bytes[1] = (900_u16 >> 8) as u8;
    let decoded = decode_fm_boot([100, 0, 0, 1], channel_bytes);
    assert_eq!(decoded.band, 0);
    assert_eq!(decoded.selected_frequency, 875);
    assert_eq!(decoded.playing_frequency, 900);
    assert_eq!(decoded.selected_channel, 0);
    assert!(decoded.memory_mode);

    let no_channels = decode_fm_boot([0x38, 0x04, 0, 1], [0xff; 40]);
    assert_eq!(no_channels.selected_frequency, 1080);
    assert_eq!(no_channels.playing_frequency, 1080);
    assert!(!no_channels.memory_mode);
}
