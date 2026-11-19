#[path = "../src/battery_indicator.rs"]
mod battery_indicator;

use battery_indicator::{FillDirection, compose_battery_indicator};

const OUTLINE: [u8; 17] = [
    0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f,
    0x30,
];
const BAR: [u8; 2] = [0xa5, 0x5a];
const BLANK: [u8; 17] = [0; 17];

fn compose(level: u8, blink: u8, direction: FillDirection) -> [u8; 17] {
    let mut output = [0xcc; 17];
    compose_battery_indicator(&mut output, &OUTLINE, &BAR, level, blink, direction);
    output
}

fn expected_filled(count: usize, direction: FillDirection) -> [u8; 17] {
    let mut expected = OUTLINE;
    let offsets = match direction {
        FillDirection::Normal => [13, 10, 7, 4],
        FillDirection::Reversed => [3, 6, 9, 12],
    };

    for offset in offsets.into_iter().take(count) {
        expected[offset..offset + 2].copy_from_slice(&BAR);
    }
    expected
}

#[test]
fn blanks_only_for_low_level_and_exactly_one_blink() {
    for level in [0, 1] {
        assert_eq!(
            compose(level, 1, FillDirection::Normal),
            BLANK,
            "level {level}"
        );
    }
}

#[test]
fn retains_the_entire_outline_when_not_filled_or_not_blinking() {
    for (level, blink) in [(0, 0), (1, 0), (1, 2), (1, u8::MAX), (2, 0), (2, 1)] {
        assert_eq!(
            compose(level, blink, FillDirection::Normal),
            OUTLINE,
            "level {level}, blink {blink}"
        );
    }
}

#[test]
fn fills_each_count_in_both_feature_selected_directions() {
    let directions = [FillDirection::Normal, FillDirection::Reversed];
    for direction in directions {
        for count in 0..=4 {
            let level = 2 + count as u8;
            assert_eq!(
                compose(level, 0, direction),
                expected_filled(count, direction),
                "{direction:?}, {count} bars"
            );
        }
    }
}

#[test]
fn saturates_out_of_range_levels_at_four_bars() {
    for direction in [FillDirection::Normal, FillDirection::Reversed] {
        assert_eq!(
            compose(u8::MAX, 0, direction),
            expected_filled(4, direction),
            "{direction:?}"
        );
    }
}

#[test]
fn non_boolean_blink_values_do_not_trigger_blanking() {
    for blink in [2, u8::MAX] {
        assert_eq!(
            compose(1, blink, FillDirection::Reversed),
            OUTLINE,
            "blink {blink}"
        );
    }
}
