use uv_k5_rust_firmware::display_boot_defaults::{
    BACKLIGHT_SUBMENU_LEN, BacklightMinStatus, DisplayBootDefaults, EEPROM_LEN, EEPROM_OFFSET,
    FeaturePresence, decode_display_boot_defaults,
};

const FEATURE_ENABLED: FeaturePresence = FeaturePresence {
    blmin_tmp_off_enabled: true,
};
const FEATURE_DISABLED: FeaturePresence = FeaturePresence {
    blmin_tmp_off_enabled: false,
};

const DEFAULTS_WITH_FEATURE: DisplayBootDefaults = DisplayBootDefaults {
    backlight_min: 0,
    backlight_max: 0,
    backlight_min_status: Some(BacklightMinStatus::On),
    channel_display_mode: 0,
    cross_band_rx_tx: 0,
    battery_save: 0,
    dual_watch: 0,
    backlight_time: 0,
    tail_tone_elimination: false,
    vfo_open: false,
};

#[test]
fn declares_the_exact_eeprom_block_and_pinned_backlight_menu_length() {
    assert_eq!(EEPROM_OFFSET, 0x0E78);
    assert_eq!(EEPROM_LEN, 8);
    assert_eq!(BACKLIGHT_SUBMENU_LEN, 8);
}

#[test]
fn decodes_valid_boundaries_and_the_packed_backlight_dependency() {
    assert_eq!(
        decode_display_boot_defaults([0x00, 0, 0, 0, 0, 0, 0, 0], FEATURE_ENABLED),
        DEFAULTS_WITH_FEATURE
    );
    assert_eq!(
        decode_display_boot_defaults([0x01, 3, 2, 4, 2, 7, 1, 1], FEATURE_ENABLED),
        DisplayBootDefaults {
            backlight_min: 0,
            backlight_max: 1,
            backlight_min_status: Some(BacklightMinStatus::On),
            channel_display_mode: 3,
            cross_band_rx_tx: 2,
            battery_save: 4,
            dual_watch: 2,
            backlight_time: 7,
            tail_tone_elimination: true,
            vfo_open: true,
        }
    );
    assert_eq!(
        decode_display_boot_defaults([0x9A, 3, 2, 4, 2, 7, 1, 1], FEATURE_ENABLED),
        DisplayBootDefaults {
            backlight_min: 9,
            backlight_max: 10,
            backlight_min_status: Some(BacklightMinStatus::On),
            channel_display_mode: 3,
            cross_band_rx_tx: 2,
            battery_save: 4,
            dual_watch: 2,
            backlight_time: 7,
            tail_tone_elimination: true,
            vfo_open: true,
        }
    );
}

#[test]
fn falls_back_at_the_first_invalid_value_for_each_field() {
    let cases = [
        (
            "backlight min below the clamped max is retained",
            [0x4B, 0, 0, 0, 0, 0, 0, 0],
            DisplayBootDefaults {
                backlight_min: 4,
                backlight_max: 10,
                ..DEFAULTS_WITH_FEATURE
            },
        ),
        (
            "backlight min equal to the clamped max is rejected",
            [0xAB, 0, 0, 0, 0, 0, 0, 0],
            DisplayBootDefaults {
                backlight_max: 10,
                ..DEFAULTS_WITH_FEATURE
            },
        ),
        (
            "backlight min equal to max",
            [0xAA, 0, 0, 0, 0, 0, 0, 0],
            DisplayBootDefaults {
                backlight_max: 10,
                ..DEFAULTS_WITH_FEATURE
            },
        ),
        (
            "backlight min greater than decoded max below the clamp",
            [0x54, 0, 0, 0, 0, 0, 0, 0],
            DisplayBootDefaults {
                backlight_max: 4,
                ..DEFAULTS_WITH_FEATURE
            },
        ),
        (
            "backlight min equal to decoded max below the clamp",
            [0x44, 0, 0, 0, 0, 0, 0, 0],
            DisplayBootDefaults {
                backlight_max: 4,
                ..DEFAULTS_WITH_FEATURE
            },
        ),
        (
            "channel display mode",
            [0, 4, 0, 0, 0, 0, 0, 0],
            DEFAULTS_WITH_FEATURE,
        ),
        (
            "cross band",
            [0, 0, 3, 0, 0, 0, 0, 0],
            DEFAULTS_WITH_FEATURE,
        ),
        (
            "battery save",
            [0, 0, 0, 5, 0, 0, 0, 0],
            DisplayBootDefaults {
                battery_save: 4,
                ..DEFAULTS_WITH_FEATURE
            },
        ),
        (
            "dual watch",
            [0, 0, 0, 0, 3, 0, 0, 0],
            DisplayBootDefaults {
                dual_watch: 1,
                ..DEFAULTS_WITH_FEATURE
            },
        ),
        (
            "backlight time",
            [0, 0, 0, 0, 0, 8, 0, 0],
            DisplayBootDefaults {
                backlight_time: 3,
                ..DEFAULTS_WITH_FEATURE
            },
        ),
        (
            "tail tone elimination",
            [0, 0, 0, 0, 0, 0, 2, 0],
            DEFAULTS_WITH_FEATURE,
        ),
        (
            "VFO open",
            [0, 0, 0, 0, 0, 0, 0, 2],
            DisplayBootDefaults {
                vfo_open: true,
                ..DEFAULTS_WITH_FEATURE
            },
        ),
    ];

    for (field, data, expected) in cases {
        assert_eq!(
            decode_display_boot_defaults(data, FEATURE_ENABLED),
            expected,
            "{field}"
        );
    }
}

#[test]
fn resets_min_when_it_is_greater_than_max() {
    assert_eq!(
        decode_display_boot_defaults([0xBA, 0, 0, 0, 0, 0, 0, 0], FEATURE_ENABLED),
        DisplayBootDefaults {
            backlight_max: 10,
            ..DEFAULTS_WITH_FEATURE
        }
    );
}

#[test]
fn erased_eeprom_uses_pinned_fallbacks() {
    assert_eq!(
        decode_display_boot_defaults([0xff; 8], FEATURE_ENABLED),
        DisplayBootDefaults {
            backlight_min: 0,
            backlight_max: 10,
            backlight_min_status: Some(BacklightMinStatus::On),
            channel_display_mode: 0,
            cross_band_rx_tx: 0,
            battery_save: 4,
            dual_watch: 1,
            backlight_time: 3,
            tail_tone_elimination: false,
            vfo_open: true,
        }
    );
}

#[test]
fn omitted_blmin_tmp_off_feature_skips_only_its_state_assignment() {
    let data = [0x9A, 3, 2, 4, 2, 7, 1, 1];
    assert_eq!(
        decode_display_boot_defaults(data, FEATURE_DISABLED),
        DisplayBootDefaults {
            backlight_min: 9,
            backlight_max: 10,
            backlight_min_status: None,
            channel_display_mode: 3,
            cross_band_rx_tx: 2,
            battery_save: 4,
            dual_watch: 2,
            backlight_time: 7,
            tail_tone_elimination: true,
            vfo_open: true,
        }
    );
    assert_eq!(
        decode_display_boot_defaults(data, FEATURE_ENABLED),
        DisplayBootDefaults {
            backlight_min: 9,
            backlight_max: 10,
            backlight_min_status: Some(BacklightMinStatus::On),
            channel_display_mode: 3,
            cross_band_rx_tx: 2,
            battery_save: 4,
            dual_watch: 2,
            backlight_time: 7,
            tail_tone_elimination: true,
            vfo_open: true,
        }
    );
}
