#![cfg_attr(not(test), no_std)]

//! Offline composition of the battery indicator from
//! `egzumer/uv-k5-firmware-custom@7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`.
//! The caller supplies the outline and bar patterns; this module performs no
//! display I/O and does not include bitmap assets.

pub const BATTERY_INDICATOR_WIDTH: usize = 17;
pub const BATTERY_BAR_WIDTH: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FillDirection {
    Normal,
    Reversed,
}

pub fn compose_battery_indicator(
    output: &mut [u8; BATTERY_INDICATOR_WIDTH],
    outline: &[u8; BATTERY_INDICATOR_WIDTH],
    bar: &[u8; BATTERY_BAR_WIDTH],
    level: u8,
    blink: u8,
    direction: FillDirection,
) {
    if level < 2 && blink == 1 {
        output.fill(0);
        return;
    }

    output.copy_from_slice(outline);
    if level <= 2 {
        return;
    }

    let bars = core::cmp::min(4, level - 2);
    for index in 0..bars {
        let index = usize::from(index);
        let offset = match direction {
            FillDirection::Normal => BATTERY_INDICATOR_WIDTH - 4 - (index * 3),
            FillDirection::Reversed => 3 + (index * 3),
        };
        output[offset..offset + BATTERY_BAR_WIDTH].copy_from_slice(bar);
    }
}
