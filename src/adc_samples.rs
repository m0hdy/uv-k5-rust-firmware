//! Offline decoding of SARADC register snapshots and battery voltage readings.
//!
//! The ADC field layout and mask priority are derived from
//! [`driver/adc.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/adc.c),
//! [`driver/adc.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/adc.h),
//! and
//! [`bsp/dp32g030/saradc.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/saradc.h)
//! at pinned upstream revision
//! [`7607f0a`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
//! Channel selection checks bits 15 through 0 and falls back to channel 0
//! when no channel bit is set. The caller supplies the STAT and DATA snapshots
//! for that selected channel; only STAT bit 0 and DATA bits 11 through 0 are
//! decoded. This module does not access registers or clear interrupts.
//!
//! The battery formula is derived from
//! [`helper/battery.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/helper/battery.c)
//! at the same pinned revision. On its 32-bit `int` target, the four `u16`
//! samples are promoted, summed, and averaged with truncating integer
//! division, then the average is multiplied by 760 and divided by the `u16`
//! calibration. The sum and product fit in `u32`; converting the result to
//! `u16` wraps modulo 2^16 as in C. A zero calibration is reported as an
//! error rather than invoking C's undefined integer division behavior.

const CHANNEL_COUNT: usize = 16;
const SAMPLE_MASK: u32 = 0x0fff;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdcRegisterSnapshot {
    pub status: u32,
    pub data: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedAdcSample {
    pub channel: u8,
    pub end_of_conversion: bool,
    pub value: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CalibrationError {
    ZeroCalibration,
}

pub fn decode_adc_sample(mask: u32, snapshot: AdcRegisterSnapshot) -> DecodedAdcSample {
    let channel = (0..CHANNEL_COUNT)
        .rev()
        .find(|channel| mask & (1u32 << channel) != 0)
        .unwrap_or(0);

    DecodedAdcSample {
        channel: channel as u8,
        end_of_conversion: snapshot.status & 1 != 0,
        value: (snapshot.data & SAMPLE_MASK) as u16,
    }
}

pub fn calibrated_battery_voltage(
    samples: [u16; 4],
    calibration: u16,
) -> Result<u16, CalibrationError> {
    if calibration == 0 {
        return Err(CalibrationError::ZeroCalibration);
    }

    let sum = samples.iter().map(|&sample| u32::from(sample)).sum::<u32>();
    let average = sum / 4;
    let calibrated = (average * 760) / u32::from(calibration);

    Ok(calibrated as u16)
}
