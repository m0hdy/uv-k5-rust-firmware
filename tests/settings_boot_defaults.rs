#[path = "../src/settings_boot_defaults.rs"]
mod settings_boot_defaults;

use settings_boot_defaults::{
    CoreSettings, EEPROM_LEN, EEPROM_OFFSET, FeaturePresence, decode_core_settings,
};

const ALL_FEATURES: FeaturePresence = FeaturePresence {
    noaa_enabled: true,
    vox_enabled: true,
};

#[test]
fn decodes_valid_minima_and_maxima() {
    assert_eq!(EEPROM_OFFSET, 0x0E70);
    assert_eq!(EEPROM_LEN, 8);

    assert_eq!(
        decode_core_settings([0, 0, 0, 0, 0, 0, 0, 0], ALL_FEATURES),
        CoreSettings {
            call_channel: 0,
            squelch_level: 0,
            tx_timeout_timer: 0,
            noaa_auto_scan: Some(false),
            key_lock: false,
            vox_switch: Some(false),
            vox_level: Some(0),
            mic_sensitivity: 0,
        }
    );
    assert_eq!(
        decode_core_settings([199, 9, 10, 1, 1, 1, 9, 4], ALL_FEATURES),
        CoreSettings {
            call_channel: 199,
            squelch_level: 9,
            tx_timeout_timer: 10,
            noaa_auto_scan: Some(true),
            key_lock: true,
            vox_switch: Some(true),
            vox_level: Some(9),
            mic_sensitivity: 4,
        }
    );
}

#[test]
fn uses_each_fields_default_at_its_first_invalid_value() {
    let cases = [
        (
            "call channel",
            [200, 0, 0, 0, 0, 0, 0, 0],
            CoreSettings {
                call_channel: 0,
                squelch_level: 0,
                tx_timeout_timer: 0,
                noaa_auto_scan: Some(false),
                key_lock: false,
                vox_switch: Some(false),
                vox_level: Some(0),
                mic_sensitivity: 0,
            },
        ),
        (
            "squelch",
            [0, 10, 0, 0, 0, 0, 0, 0],
            CoreSettings {
                call_channel: 0,
                squelch_level: 1,
                tx_timeout_timer: 0,
                noaa_auto_scan: Some(false),
                key_lock: false,
                vox_switch: Some(false),
                vox_level: Some(0),
                mic_sensitivity: 0,
            },
        ),
        (
            "timeout",
            [0, 0, 11, 0, 0, 0, 0, 0],
            CoreSettings {
                call_channel: 0,
                squelch_level: 0,
                tx_timeout_timer: 1,
                noaa_auto_scan: Some(false),
                key_lock: false,
                vox_switch: Some(false),
                vox_level: Some(0),
                mic_sensitivity: 0,
            },
        ),
        (
            "NOAA auto scan",
            [0, 0, 0, 2, 0, 0, 0, 0],
            CoreSettings {
                call_channel: 0,
                squelch_level: 0,
                tx_timeout_timer: 0,
                noaa_auto_scan: Some(false),
                key_lock: false,
                vox_switch: Some(false),
                vox_level: Some(0),
                mic_sensitivity: 0,
            },
        ),
        (
            "key lock",
            [0, 0, 0, 0, 2, 0, 0, 0],
            CoreSettings {
                call_channel: 0,
                squelch_level: 0,
                tx_timeout_timer: 0,
                noaa_auto_scan: Some(false),
                key_lock: false,
                vox_switch: Some(false),
                vox_level: Some(0),
                mic_sensitivity: 0,
            },
        ),
        (
            "VOX switch",
            [0, 0, 0, 0, 0, 2, 0, 0],
            CoreSettings {
                call_channel: 0,
                squelch_level: 0,
                tx_timeout_timer: 0,
                noaa_auto_scan: Some(false),
                key_lock: false,
                vox_switch: Some(false),
                vox_level: Some(0),
                mic_sensitivity: 0,
            },
        ),
        (
            "VOX level",
            [0, 0, 0, 0, 0, 0, 10, 0],
            CoreSettings {
                call_channel: 0,
                squelch_level: 0,
                tx_timeout_timer: 0,
                noaa_auto_scan: Some(false),
                key_lock: false,
                vox_switch: Some(false),
                vox_level: Some(1),
                mic_sensitivity: 0,
            },
        ),
        (
            "microphone sensitivity",
            [0, 0, 0, 0, 0, 0, 0, 5],
            CoreSettings {
                call_channel: 0,
                squelch_level: 0,
                tx_timeout_timer: 0,
                noaa_auto_scan: Some(false),
                key_lock: false,
                vox_switch: Some(false),
                vox_level: Some(0),
                mic_sensitivity: 4,
            },
        ),
    ];

    for (field, data, expected) in cases {
        assert_eq!(
            decode_core_settings(data, ALL_FEATURES),
            expected,
            "{field}"
        );
    }
}

#[test]
fn erased_eeprom_uses_pinned_defaults() {
    assert_eq!(
        decode_core_settings([0xff; 8], ALL_FEATURES),
        CoreSettings {
            call_channel: 0,
            squelch_level: 1,
            tx_timeout_timer: 1,
            noaa_auto_scan: Some(false),
            key_lock: false,
            vox_switch: Some(false),
            vox_level: Some(1),
            mic_sensitivity: 4,
        }
    );
}

#[test]
fn represents_all_noaa_and_vox_presence_combinations() {
    let data = [73, 8, 10, 1, 1, 1, 9, 4];
    let cases = [
        (
            FeaturePresence {
                noaa_enabled: false,
                vox_enabled: false,
            },
            None,
            None,
            None,
        ),
        (
            FeaturePresence {
                noaa_enabled: true,
                vox_enabled: false,
            },
            Some(true),
            None,
            None,
        ),
        (
            FeaturePresence {
                noaa_enabled: false,
                vox_enabled: true,
            },
            None,
            Some(true),
            Some(9),
        ),
        (
            FeaturePresence {
                noaa_enabled: true,
                vox_enabled: true,
            },
            Some(true),
            Some(true),
            Some(9),
        ),
    ];

    for (features, noaa_auto_scan, vox_switch, vox_level) in cases {
        assert_eq!(
            decode_core_settings(data, features),
            CoreSettings {
                call_channel: 73,
                squelch_level: 8,
                tx_timeout_timer: 10,
                noaa_auto_scan,
                key_lock: true,
                vox_switch,
                vox_level,
                mic_sensitivity: 4,
            }
        );
    }
}
