/*
 * Copyright 2023 Dual Tachyon
 * https://github.com/DualTachyon
 *
 * This file contains portions derived from frequencies.c, dcs.c, and radio.c
 * in the pinned uv-k5-firmware-custom reference at
 * 7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

pub const STEP_FREQUENCIES: [u16; 24] = [
    250, 500, 625, 1000, 1250, 2500, 833, 1, 5, 10, 25, 50, 100, 125, 900, 1500, 2000, 3000, 5000,
    10000, 12500, 20000, 25000, 50000,
];

pub const SORTED_STEP_INDEXES: [u8; 24] = [
    7, 8, 9, 10, 11, 12, 13, 0, 1, 2, 6, 14, 3, 4, 15, 16, 5, 17, 18, 19, 20, 21, 22, 23,
];

pub const fn step_frequency_ticks(index: usize) -> Option<u16> {
    if index < STEP_FREQUENCIES.len() {
        Some(STEP_FREQUENCIES[index])
    } else {
        None
    }
}

pub const fn sorted_step_frequency_ticks(sorted_index: usize) -> Option<u16> {
    if sorted_index < SORTED_STEP_INDEXES.len() {
        Some(STEP_FREQUENCIES[SORTED_STEP_INDEXES[sorted_index] as usize])
    } else {
        None
    }
}

// ENABLE_WIDE_RX is enabled in the pinned reference configuration.
const BANDS: [(u32, u32); 7] = [
    (1_800_000, 10_800_000),
    (10_800_000, 13_700_000),
    (13_700_000, 17_400_000),
    (17_400_000, 35_000_000),
    (35_000_000, 40_000_000),
    (40_000_000, 47_000_000),
    (47_000_000, 130_000_000),
];

const CHIP_BAND1_UPPER: u32 = 63_000_000;
const CHIP_BAND2_LOWER: u32 = 84_000_000;

pub const fn rx_frequency_check(frequency: u32) -> i32 {
    if frequency < BANDS[0].0
        || frequency > BANDS[BANDS.len() - 1].1
        || (frequency >= CHIP_BAND1_UPPER && frequency < CHIP_BAND2_LOWER)
    {
        -1
    } else {
        0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TxLockMode {
    Default,
    Fcc,
    Ce,
    Gb,
    Amateur430,
    Amateur438,
    All,
    None,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TxPermissions {
    pub band_200: bool,
    pub band_350: bool,
    pub band_350_enabled: bool,
    pub band_500: bool,
}

pub const fn tx_frequency_check(
    frequency: u32,
    lock: TxLockMode,
    permissions: TxPermissions,
) -> i32 {
    if frequency < BANDS[0].0 || frequency > BANDS[BANDS.len() - 1].1 {
        return 1;
    }
    if frequency >= CHIP_BAND1_UPPER && frequency < CHIP_BAND2_LOWER {
        return -1;
    }

    let allowed = match lock {
        TxLockMode::Default => {
            (frequency >= BANDS[2].0 && frequency < BANDS[2].1)
                || (permissions.band_200 && frequency >= BANDS[3].0 && frequency < BANDS[3].1)
                || (permissions.band_350
                    && permissions.band_350_enabled
                    && frequency >= BANDS[4].0
                    && frequency < BANDS[4].1)
                || (frequency >= BANDS[5].0 && frequency < BANDS[5].1)
                || (permissions.band_500 && frequency >= BANDS[6].0 && frequency <= 60_000_000)
        }
        TxLockMode::Fcc => {
            (frequency >= 14_400_000 && frequency < 14_800_000)
                || (frequency >= 42_000_000 && frequency < 45_000_000)
        }
        TxLockMode::Ce => {
            (frequency >= 14_400_000 && frequency < 14_600_000)
                || (frequency >= 43_000_000 && frequency < 44_000_000)
        }
        TxLockMode::Gb => {
            (frequency >= 14_400_000 && frequency < 14_800_000)
                || (frequency >= 43_000_000 && frequency < 44_000_000)
        }
        TxLockMode::Amateur430 => {
            (frequency >= BANDS[2].0 && frequency < BANDS[2].1)
                || (frequency >= BANDS[5].0 && frequency < 43_000_000)
        }
        TxLockMode::Amateur438 => {
            (frequency >= BANDS[2].0 && frequency < BANDS[2].1)
                || (frequency >= BANDS[5].0 && frequency < 43_800_000)
        }
        TxLockMode::All => false,
        TxLockMode::None => {
            let mut band = 0;
            let mut in_band = false;
            while band < BANDS.len() {
                if frequency >= BANDS[band].0 && frequency < BANDS[band].1 {
                    in_band = true;
                    break;
                }
                band += 1;
            }
            in_band
        }
    };

    if allowed { 0 } else { -1 }
}

pub const fn round_to_step(frequency: u32, mut step: u16) -> Option<u32> {
    if step == 0 {
        return None;
    }
    if step == 833 {
        let base = frequency / 2500 * 2500;
        let channel = (frequency - base) / 700;
        let rounded = base as u64 + (channel * 833) as u64 + (channel == 3) as u64;
        return if rounded <= u32::MAX as u64 {
            Some(rounded as u32)
        } else {
            None
        };
    }
    if step == 1 {
        return Some(frequency);
    }
    if step >= 1000 {
        step /= 2;
    }

    let rounded = (frequency as u64 + step.div_ceil(2) as u64) / step as u64 * step as u64;
    if rounded <= u32::MAX as u64 {
        Some(rounded as u32)
    } else {
        None
    }
}

const CTCSS_OPTIONS: [u16; 50] = [
    670, 693, 719, 744, 770, 797, 825, 854, 885, 915, 948, 974, 1000, 1035, 1072, 1109, 1148, 1188,
    1230, 1273, 1318, 1365, 1413, 1462, 1514, 1567, 1598, 1622, 1655, 1679, 1713, 1738, 1773, 1799,
    1835, 1862, 1899, 1928, 1966, 1995, 2035, 2065, 2107, 2181, 2257, 2291, 2336, 2418, 2503, 2541,
];

pub fn ctcss_code(tenths_hz: i32) -> u8 {
    let mut result = u8::MAX;
    let mut smallest_delta = CTCSS_OPTIONS.len() as i64;

    for (index, option) in CTCSS_OPTIONS.iter().enumerate() {
        let delta = (tenths_hz as i64 - *option as i64).abs();
        if delta < smallest_delta {
            smallest_delta = delta;
            result = index as u8;
        }
    }

    result
}

const DCS_OPTIONS: [u16; 104] = [
    0x0013, 0x0015, 0x0016, 0x0019, 0x001a, 0x001e, 0x0023, 0x0027, 0x0029, 0x002b, 0x002c, 0x0035,
    0x0039, 0x003a, 0x003b, 0x003c, 0x004c, 0x004d, 0x004e, 0x0052, 0x0055, 0x0059, 0x005a, 0x005c,
    0x0063, 0x0065, 0x006a, 0x006d, 0x006e, 0x0072, 0x0075, 0x007a, 0x007c, 0x0085, 0x008a, 0x0093,
    0x0095, 0x0096, 0x00a3, 0x00a4, 0x00a5, 0x00a6, 0x00a9, 0x00aa, 0x00ad, 0x00b1, 0x00b3, 0x00b5,
    0x00b6, 0x00b9, 0x00bc, 0x00c6, 0x00c9, 0x00cd, 0x00d5, 0x00d9, 0x00da, 0x00e3, 0x00e6, 0x00e9,
    0x00ee, 0x00f4, 0x00f5, 0x00f9, 0x0109, 0x010a, 0x010b, 0x0113, 0x0119, 0x011a, 0x0125, 0x0126,
    0x012a, 0x012c, 0x012d, 0x0132, 0x0134, 0x0135, 0x0136, 0x0143, 0x0146, 0x014e, 0x0153, 0x0156,
    0x015a, 0x0166, 0x0175, 0x0186, 0x018a, 0x0194, 0x0197, 0x0199, 0x019a, 0x01ac, 0x01b2, 0x01b4,
    0x01c3, 0x01ca, 0x01d3, 0x01d9, 0x01da, 0x01dc, 0x01e3, 0x01ec,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DcsCodeType {
    Normal,
    Reverse,
}

fn calculate_golay(code_word: u32) -> u32 {
    let mut word = code_word;
    for _ in 0..12 {
        word <<= 1;
        if word & 0x1000 != 0 {
            word ^= 0x08ea;
        }
    }
    code_word | ((word & 0x0ffe) << 11)
}

pub fn dcs_code_word(code_type: DcsCodeType, option: usize) -> Option<u32> {
    let code = calculate_golay(*DCS_OPTIONS.get(option)? as u32 + 0x800);
    Some(match code_type {
        DcsCodeType::Normal => code,
        DcsCodeType::Reverse => code ^ 0x7f_ffff,
    })
}

pub fn decode_dcs_code(code: u32) -> u8 {
    let mut code = code;
    for _ in 0..23 {
        if (code >> 9) & 0x7 == 4 {
            let option = code & 0x1ff;
            if let Some((index, _)) = DCS_OPTIONS
                .iter()
                .enumerate()
                .find(|(_, candidate)| **candidate as u32 == option)
                && dcs_code_word(DcsCodeType::Normal, index) == Some(code)
            {
                return index as u8;
            }
        }

        let mut shifted = code >> 1;
        if code & 1 != 0 {
            shifted |= 0x40_0000;
        }
        code = shifted;
    }

    u8::MAX
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SquelchThresholds {
    pub open_rssi: u8,
    pub close_rssi: u8,
    pub open_noise: u8,
    pub close_noise: u8,
    pub close_glitch: u8,
    pub open_glitch: u8,
}

pub const SQUELCH_OFF: SquelchThresholds = SquelchThresholds {
    open_rssi: 0,
    close_rssi: 0,
    open_noise: 127,
    close_noise: 127,
    close_glitch: 255,
    open_glitch: 255,
};

fn clamp_threshold(value: u16, maximum: u16) -> u8 {
    if value > maximum {
        maximum as u8
    } else {
        value as u8
    }
}

pub fn configure_squelch(
    level: u8,
    stored: SquelchThresholds,
    more_sensitive: bool,
) -> SquelchThresholds {
    if level == 0 {
        return SQUELCH_OFF;
    }

    let mut open_rssi = stored.open_rssi as u16;
    let mut close_rssi = stored.close_rssi as u16;
    let mut open_noise = stored.open_noise as u16;
    let mut close_noise = stored.close_noise as u16;
    let mut close_glitch = stored.close_glitch as u16;
    let mut open_glitch = stored.open_glitch as u16;

    if more_sensitive {
        open_rssi /= 2;
        open_noise *= 2;
        open_glitch *= 2;

        if close_rssi == open_rssi && close_rssi >= 2 {
            close_rssi -= 2;
        }
        if close_noise == open_noise && close_noise <= 125 {
            close_noise += 2;
        }
        if close_glitch == open_glitch && close_glitch <= 253 {
            close_glitch += 2;
        }
    }

    SquelchThresholds {
        open_rssi: clamp_threshold(open_rssi, 255),
        close_rssi: clamp_threshold(close_rssi, 255),
        open_noise: clamp_threshold(open_noise, 127),
        close_noise: clamp_threshold(close_noise, 127),
        close_glitch: clamp_threshold(close_glitch, 255),
        open_glitch: clamp_threshold(open_glitch, 255),
    }
}
