#[path = "../src/tuning_signalling.rs"]
mod tuning_signalling;

use tuning_signalling::{
    DcsCodeType, SORTED_STEP_INDEXES, SQUELCH_OFF, STEP_FREQUENCIES, SquelchThresholds, TxLockMode,
    TxPermissions, configure_squelch, ctcss_code, dcs_code_word, decode_dcs_code, round_to_step,
    rx_frequency_check, sorted_step_frequency_ticks, step_frequency_ticks, tx_frequency_check,
};

#[test]
fn pinned_rx_frequency_vectors_match() {
    for (frequency, expected) in [
        (1_799_999, -1),   // frequency/rx_below_lower
        (1_800_000, 0),    // frequency/rx_at_lower
        (63_000_000, -1),  // frequency/rx_chip_gap_lower
        (84_000_000, 0),   // frequency/rx_at_chip_gap_upper
        (130_000_000, 0),  // frequency/rx_at_upper
        (130_000_001, -1), // frequency/rx_above_upper
    ] {
        assert_eq!(rx_frequency_check(frequency), expected);
    }
}

#[test]
fn receive_frequency_boundaries_and_gap_are_exact() {
    assert_eq!(rx_frequency_check(1_799_999), -1);
    assert_eq!(rx_frequency_check(1_800_000), 0);
    assert_eq!(rx_frequency_check(62_999_999), 0);
    assert_eq!(rx_frequency_check(63_000_000), -1);
    assert_eq!(rx_frequency_check(83_999_999), -1);
    assert_eq!(rx_frequency_check(84_000_000), 0);
    assert_eq!(rx_frequency_check(130_000_000), 0);
    assert_eq!(rx_frequency_check(130_000_001), -1);
    assert_eq!(rx_frequency_check(u32::MAX), -1);
}

#[test]
fn pinned_tx_frequency_vectors_match() {
    let defaults = TxPermissions::default();
    for (frequency, lock, permissions, expected) in [
        (13_700_000, TxLockMode::Default, defaults, 0), // tx_default_lower
        (17_399_999, TxLockMode::Default, defaults, 0), // tx_default_last_tick
        (17_400_000, TxLockMode::Default, defaults, -1), // tx_default_next_band
        (14_400_000, TxLockMode::Fcc, defaults, 0),     // tx_fcc_lower
        (14_800_000, TxLockMode::Fcc, defaults, -1),    // tx_fcc_upper
        (17_400_000, TxLockMode::None, defaults, 0),    // tx_unlocked_band_edge
        (14_400_000, TxLockMode::All, defaults, -1),    // tx_all_locked
    ] {
        assert_eq!(
            tx_frequency_check(frequency, lock, permissions),
            expected,
            "frequency {frequency}, lock {lock:?}"
        );
    }
}

#[test]
fn transmit_lock_modes_cover_band_edges_and_invalid_range() {
    let permissions = TxPermissions {
        band_200: true,
        band_350: true,
        band_350_enabled: true,
        band_500: true,
    };
    assert_eq!(
        tx_frequency_check(35_000_000, TxLockMode::Default, permissions),
        0
    );
    assert_eq!(
        tx_frequency_check(40_000_000, TxLockMode::Default, permissions),
        0
    );
    assert_eq!(
        tx_frequency_check(60_000_000, TxLockMode::Default, permissions),
        0
    );
    assert_eq!(
        tx_frequency_check(60_000_001, TxLockMode::Default, permissions),
        -1
    );
    assert_eq!(
        tx_frequency_check(1_799_999, TxLockMode::None, permissions),
        1
    );
    assert_eq!(
        tx_frequency_check(63_000_000, TxLockMode::None, permissions),
        -1
    );
    assert_eq!(
        tx_frequency_check(130_000_001, TxLockMode::None, permissions),
        1
    );
    assert_eq!(
        tx_frequency_check(14_599_999, TxLockMode::Ce, permissions),
        0
    );
    assert_eq!(
        tx_frequency_check(14_600_000, TxLockMode::Ce, permissions),
        -1
    );
    assert_eq!(
        tx_frequency_check(43_799_999, TxLockMode::Amateur438, permissions),
        0
    );
    assert_eq!(
        tx_frequency_check(43_800_000, TxLockMode::Amateur438, permissions),
        -1
    );
    assert_eq!(
        tx_frequency_check(43_000_000, TxLockMode::Gb, permissions),
        0
    );
    assert_eq!(
        tx_frequency_check(42_999_999, TxLockMode::Amateur430, permissions),
        0
    );
    assert_eq!(
        tx_frequency_check(43_000_000, TxLockMode::Amateur430, permissions),
        -1
    );
}

#[test]
fn step_table_and_sorted_order_match_the_pinned_indices() {
    assert_eq!(STEP_FREQUENCIES.len(), 24);
    assert_eq!(SORTED_STEP_INDEXES.len(), 24);
    assert_eq!(step_frequency_ticks(0), Some(250));
    assert_eq!(step_frequency_ticks(6), Some(833));
    assert_eq!(step_frequency_ticks(23), Some(50_000));
    assert_eq!(step_frequency_ticks(24), None);
    assert_eq!(sorted_step_frequency_ticks(0), Some(1));
    assert_eq!(sorted_step_frequency_ticks(10), Some(833));
    assert_eq!(sorted_step_frequency_ticks(23), Some(50_000));
    assert_eq!(sorted_step_frequency_ticks(24), None);
}

#[test]
fn pinned_rounding_vectors_and_invalid_steps_match() {
    assert_eq!(round_to_step(14_500_125, 250), Some(14_500_250)); // round_half_step
    assert_eq!(round_to_step(14_502_000, 833), Some(14_501_666)); // round_833_channel
    assert_eq!(round_to_step(14_500_125, 0), None);
    assert_eq!(round_to_step(14_500_123, 1), Some(14_500_123));
    assert_eq!(round_to_step(1_250, 1_000), Some(1_500));
    assert_eq!(round_to_step(u32::MAX, 2), None);
    assert_eq!(round_to_step(u32::MAX, 833), None);
}

#[test]
fn pinned_ctcss_vectors_and_nearest_tone_limit_match() {
    assert_eq!(ctcss_code(670), 0); // ctcss/first_tone
    assert_eq!(ctcss_code(2541), 49); // ctcss/last_tone
    assert_eq!(ctcss_code(-1), u8::MAX); // ctcss/below_range
    assert_eq!(ctcss_code(9999), u8::MAX); // ctcss/above_range
    assert_eq!(ctcss_code(706), 1); // Equal-distance tie selects the earlier option.
    assert_eq!(ctcss_code(718), 2);
    assert_eq!(ctcss_code(720), 2);
    assert_eq!(ctcss_code(620), u8::MAX);
    assert_eq!(ctcss_code(621), 0);
    assert_eq!(ctcss_code(i32::MIN), u8::MAX);
    assert_eq!(ctcss_code(i32::MAX), u8::MAX);
}

#[test]
fn pinned_dcs_vectors_and_invalid_options_match() {
    assert_eq!(dcs_code_word(DcsCodeType::Normal, 0), Some(7_747_603)); // dcs/first_normal_word
    assert_eq!(dcs_code_word(DcsCodeType::Normal, 103), Some(2_161_132)); // dcs/last_normal_word
    assert_eq!(dcs_code_word(DcsCodeType::Reverse, 0), Some(641_004)); // dcs/first_reverse_word
    assert_eq!(decode_dcs_code(641_004), 7); // dcs/decode_first_reverse_word
    assert_eq!(decode_dcs_code(0), u8::MAX); // dcs/decode_invalid_word
    assert_eq!(dcs_code_word(DcsCodeType::Normal, 104), None);
    assert_eq!(dcs_code_word(DcsCodeType::Reverse, usize::MAX), None);
}

#[test]
fn squelch_off_matches_source_defaults() {
    let stored = SquelchThresholds {
        open_rssi: 50,
        close_rssi: 40,
        open_noise: 65,
        close_noise: 70,
        close_glitch: 90,
        open_glitch: 100,
    };
    assert_eq!(configure_squelch(0, stored, true), SQUELCH_OFF);
}

#[test]
fn more_sensitive_squelch_adjusts_equal_thresholds() {
    let stored = SquelchThresholds {
        open_rssi: 20,
        close_rssi: 10,
        open_noise: 35,
        close_noise: 70,
        close_glitch: 100,
        open_glitch: 50,
    };
    assert_eq!(
        configure_squelch(5, stored, true),
        SquelchThresholds {
            open_rssi: 10,
            close_rssi: 8,
            open_noise: 70,
            close_noise: 72,
            close_glitch: 102,
            open_glitch: 100,
        }
    );
}

#[test]
fn more_sensitive_squelch_saturates_scaled_thresholds() {
    let stored = SquelchThresholds {
        open_rssi: 20,
        close_rssi: 10,
        open_noise: 70,
        close_noise: 140,
        close_glitch: 255,
        open_glitch: 200,
    };
    assert_eq!(
        configure_squelch(5, stored, true),
        SquelchThresholds {
            open_rssi: 10,
            close_rssi: 8,
            open_noise: 127,
            close_noise: 127,
            close_glitch: 255,
            open_glitch: 255,
        }
    );
}

#[test]
fn normal_squelch_preserves_rssi_and_glitch_and_clamps_noise() {
    let stored = SquelchThresholds {
        open_rssi: 50,
        close_rssi: 40,
        open_noise: 200,
        close_noise: 128,
        close_glitch: 90,
        open_glitch: 100,
    };
    assert_eq!(
        configure_squelch(5, stored, false),
        SquelchThresholds {
            open_rssi: 50,
            close_rssi: 40,
            open_noise: 127,
            close_noise: 127,
            close_glitch: 90,
            open_glitch: 100,
        }
    );
}

// The pinned fixture has no squelch threshold or detection vectors. These cases
// exercise the source-derived pure transformation only; EEPROM tables, RF
// detection behavior, and hardware parity remain explicitly unverified.
