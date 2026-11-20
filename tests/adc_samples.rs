use uv_k5_rust_firmware::adc_samples::{self,
    AdcRegisterSnapshot, CalibrationError, calibrated_battery_voltage, decode_adc_sample,
};

#[test]
fn selects_highest_channel_bit_and_defaults_to_channel_zero() {
    let snapshot = AdcRegisterSnapshot { status: 0, data: 0 };
    let cases = [
        (0x0000, 0),
        (0x0001, 0),
        (0x8000, 15),
        (0x8025, 15),
        (0x10001, 0),
        (0x1_0000, 0),
    ];

    for (mask, expected_channel) in cases {
        assert_eq!(
            decode_adc_sample(mask, snapshot).channel,
            expected_channel,
            "mask {mask:#07x}"
        );
    }
}

#[test]
fn decodes_eoc_and_twelve_bit_sample_without_unrelated_register_bits() {
    let not_complete = decode_adc_sample(
        1 << 9,
        AdcRegisterSnapshot {
            status: 0xffff_fffe,
            data: 0xffff_f000,
        },
    );
    assert_eq!(
        not_complete,
        adc_samples::DecodedAdcSample {
            channel: 9,
            end_of_conversion: false,
            value: 0,
        }
    );

    let complete = decode_adc_sample(
        1 << 4,
        AdcRegisterSnapshot {
            status: 0xffff_ffff,
            data: 0xabc0_0fff,
        },
    );
    assert_eq!(
        complete,
        adc_samples::DecodedAdcSample {
            channel: 4,
            end_of_conversion: true,
            value: 4095,
        }
    );
}

#[test]
fn averages_four_samples_with_truncating_division() {
    assert_eq!(calibrated_battery_voltage([1, 1, 1, 2], 1), Ok(760));
}

#[test]
fn truncates_calibration_division_and_final_u16_assignment() {
    assert_eq!(calibrated_battery_voltage([1; 4], 3), Ok(253));
    assert_eq!(calibrated_battery_voltage([u16::MAX; 4], 1), Ok(64776));
}

#[test]
fn handles_adc_scale_and_maximum_calibration() {
    assert_eq!(calibrated_battery_voltage([4095; 4], u16::MAX), Ok(47));
    assert_eq!(calibrated_battery_voltage([4095; 4], 1), Ok(32008));
}

#[test]
fn rejects_zero_calibration() {
    assert_eq!(
        calibrated_battery_voltage([0; 4], 0),
        Err(CalibrationError::ZeroCalibration)
    );
}
