#![no_std]

use uv_k5_rust_firmware::{
    channel_codec::{ChannelRecord, Settings},
    channel_validity::{ChannelAttributes, is_valid_channel},
    scan_decisions::{
        FunctionState, ResumeMode, ScanAction, ScanEvent, ScanKind, ScanState, StopDisposition,
    },
    tuning_signalling::{
        DcsCodeType, SquelchThresholds, TxLockMode, TxPermissions, configure_squelch, ctcss_code,
        dcs_code_word, decode_dcs_code, round_to_step, rx_frequency_check, tx_frequency_check,
    },
};

#[inline(never)]
pub fn exercise() -> u32 {
    let settings = Settings {
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
    };

    let mut checksum = 0u32;
    if let Ok(image) = settings.encode() {
        for block in &image.blocks {
            for byte in block {
                checksum = checksum.rotate_left(5) ^ u32::from(*byte);
            }
        }
        if let Ok(decoded) = image.decode() {
            checksum ^= u32::from(decoded.squelch_level);
            checksum ^= u32::from(decoded.scan_resume_mode) << 8;
        }
    }

    let channel = ChannelRecord {
        rx_frequency: 145_500_000,
        tx_offset: 600_000,
        rx_code: 1,
        tx_code: 2,
        rx_code_type: 3,
        tx_code_type: 4,
        tx_offset_direction: 1,
        modulation: 2,
        frequency_reverse: false,
        channel_bandwidth: true,
        output_power: 2,
        busy_channel_lock: 1,
        dtmf_decoding_enabled: true,
        dtmf_ptt_id_tx_mode: 3,
        step_setting: 5,
        scrambling_type: 1,
    };
    if let Ok(bytes) = channel.encode() {
        checksum ^= u32::from(bytes[10]) << 16;
        if let Ok(decoded) = ChannelRecord::decode(&bytes) {
            checksum ^= decoded.rx_frequency;
            checksum ^= decoded.tx_offset.rotate_left(3);
        }
    }

    checksum ^= round_to_step(14_500_125, 250).unwrap_or_default();
    checksum ^= rx_frequency_check(1_800_000) as u32;
    checksum ^= (tx_frequency_check(14_400_000, TxLockMode::Fcc, TxPermissions::default()) as u32)
        .rotate_left(1);
    checksum ^= u32::from(ctcss_code(706)) << 8;
    if let Some(word) = dcs_code_word(DcsCodeType::Normal, 0) {
        checksum ^= word;
        checksum ^= u32::from(decode_dcs_code(word)) << 24;
    }
    let squelch = configure_squelch(
        5,
        SquelchThresholds {
            open_rssi: 20,
            close_rssi: 10,
            open_noise: 35,
            close_noise: 70,
            close_glitch: 100,
            open_glitch: 50,
        },
        true,
    );
    checksum ^= u32::from(squelch.open_rssi)
        | (u32::from(squelch.close_noise) << 8)
        | (u32::from(squelch.open_glitch) << 16);

    let mut scan = ScanState::new();
    checksum ^= match scan.apply(ScanEvent::Start {
        kind: ScanKind::Memory,
        resume_mode: ResumeMode::Time,
        store_backup_settings: true,
    }) {
        ScanAction::Advance => 0x10,
        _ => 0,
    };
    scan.apply(ScanEvent::Tick {
        function: FunctionState::Incoming,
    });
    scan.apply(ScanEvent::Detected);
    scan.apply(ScanEvent::Found);
    checksum ^= u32::from(scan.remaining_10ms());
    checksum ^= u32::from(scan.is_active()) << 16;
    checksum ^= match scan.apply(ScanEvent::Stop) {
        ScanAction::Stopped(StopDisposition::KeepLastFound) => 0x20,
        ScanAction::Stopped(StopDisposition::RestoreInitial) => 0x40,
        _ => 0,
    };

    let attributes = ChannelAttributes {
        band: 2,
        scan_list_1: true,
        scan_list_2: false,
    };
    checksum ^= u32::from(is_valid_channel(2, attributes, true, 0, 10, 11));
    checksum ^= u32::from(is_valid_channel(10, attributes, true, 0, 10, 11)) << 1;
    checksum
}
