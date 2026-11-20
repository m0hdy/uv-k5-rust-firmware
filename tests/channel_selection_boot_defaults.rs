#[path = "../src/channel_selection_boot_defaults.rs"]
mod channel_selection_boot_defaults;

use channel_selection_boot_defaults::{
    ChannelSelectionBootDefaults, EEPROM_LEN, EEPROM_OFFSET, decode_channel_selection_boot_defaults,
};

const fn expected(
    screen_channel: [u8; 2],
    mr_channel: [u8; 2],
    freq_channel: [u8; 2],
    noaa_channel: Option<[u8; 2]>,
) -> ChannelSelectionBootDefaults {
    ChannelSelectionBootDefaults {
        screen_channel,
        mr_channel,
        freq_channel,
        noaa_channel,
    }
}

const DEFAULTS_WITH_NOAA: ChannelSelectionBootDefaults =
    expected([205, 205], [0, 0], [205, 205], Some([207, 207]));

#[test]
fn decodes_distinct_vfo_positions_and_preserves_u8_values() {
    assert_eq!(EEPROM_OFFSET, 0x0E80);
    assert_eq!(EEPROM_LEN, 8);
    assert_eq!(
        decode_channel_selection_boot_defaults([37, 173, 203, 91, 4, 204, 211, 216], true),
        expected([37, 91], [173, 4], [203, 204], Some([211, 216]))
    );
}

#[test]
fn validates_raw_channel_interval_endpoints_and_adjacent_values() {
    let cases = [
        (
            [0, 0, 200, 0, 0, 206, 207, 216],
            expected([0, 0], [0, 0], [200, 206], Some([207, 216])),
        ),
        (
            [216, 199, 200, 216, 199, 206, 207, 216],
            expected([216, 216], [199, 199], [200, 206], Some([207, 216])),
        ),
        ([217, 200, 199, 217, 200, 207, 206, 217], DEFAULTS_WITH_NOAA),
    ];

    for (data, result) in cases {
        assert_eq!(decode_channel_selection_boot_defaults(data, true), result);
    }
}

#[test]
fn rejects_cross_class_values_using_each_field_specific_interval() {
    assert_eq!(
        decode_channel_selection_boot_defaults([207, 200, 207, 206, 199, 207, 200, 206], true),
        expected([207, 206], [0, 199], [205, 205], Some([207, 207]))
    );
}

#[test]
fn erased_and_extreme_bytes_use_pinned_defaults() {
    assert_eq!(
        decode_channel_selection_boot_defaults([0xff; 8], true),
        DEFAULTS_WITH_NOAA
    );
    assert_eq!(
        decode_channel_selection_boot_defaults([0xff; 8], false),
        expected([205, 205], [0, 0], [205, 205], None)
    );
}

#[test]
fn noaa_channels_are_present_only_when_enabled() {
    let data = [1, 2, 201, 3, 4, 202, 209, 210];

    assert_eq!(
        decode_channel_selection_boot_defaults(data, true),
        expected([1, 3], [2, 4], [201, 202], Some([209, 210]))
    );
    assert_eq!(
        decode_channel_selection_boot_defaults(data, false),
        expected([1, 3], [2, 4], [201, 202], None)
    );
}

#[test]
fn invalid_noaa_neighbors_default_independently() {
    assert_eq!(
        decode_channel_selection_boot_defaults([0, 0, 0, 0, 0, 0, 206, 217], true),
        expected([0, 0], [0, 0], [205, 205], Some([207, 207]))
    );
}
