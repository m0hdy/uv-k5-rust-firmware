use uv_k5_rust_firmware::backlight_plan::{
    self, BacklightInitPlan, BacklightSnapshot, BacklightWrite, Register, WriteKind,
    brightness_compare_value,
};

fn write(register: Register, kind: WriteKind, value: u32) -> BacklightWrite {
    BacklightWrite {
        register,
        kind,
        value,
    }
}

#[test]
fn trace_preserves_six_source_writes_and_or_clear_set_values() {
    let writes: Vec<_> = BacklightInitPlan::from_snapshot(BacklightSnapshot {
        pwm_plus0_clksrc: 0x1234_5678,
        portb_sel0: 0x89ab_cdef,
    })
    .writes()
    .collect();

    assert_eq!(
        writes,
        vec![
            write(
                Register::PwmPlus0ClockSource,
                WriteKind::Or { bits: 0x002e_0000 },
                0x123e_5678,
            ),
            write(Register::PwmPlus0Period, WriteKind::Assign, 1023),
            write(
                Register::PortBSel0,
                WriteKind::Clear { mask: 0x0f00_0000 },
                0x80ab_cdef,
            ),
            write(
                Register::PortBSel0,
                WriteKind::Set { bits: 0x0100_0000 },
                0x81ab_cdef,
            ),
            write(Register::PwmPlus0Generator, WriteKind::Assign, 0x0101_0000),
            write(Register::PwmPlus0Config, WriteKind::Assign, 0x0000_0005),
        ]
    );
}

#[test]
fn different_snapshot_changes_only_the_dependent_values() {
    let writes: Vec<_> = BacklightInitPlan::from_snapshot(BacklightSnapshot {
        pwm_plus0_clksrc: 0x8000_0001,
        portb_sel0: 0x0fff_ffff,
    })
    .writes()
    .collect();

    assert_eq!(writes.len(), 6);
    assert_eq!(writes[0].value, 0x802e_0001);
    assert_eq!(writes[1].value, 1023);
    assert_eq!(writes[2].value, 0x00ff_ffff);
    assert_eq!(writes[3].value, 0x01ff_ffff);
    assert_eq!(writes[4].value, 0x0101_0000);
    assert_eq!(writes[5].value, 5);
}

#[test]
fn brightness_compare_matches_supported_settings_range() {
    let expected = [0, 1, 3, 7, 15, 31, 63, 127, 255, 511, 1023];

    for (brightness, compare) in expected.into_iter().enumerate() {
        assert_eq!(brightness_compare_value(brightness as u8), Ok(compare));
    }
}

#[test]
fn brightness_outside_settings_range_is_rejected() {
    assert_eq!(
        brightness_compare_value(11),
        Err(backlight_plan::BrightnessOutOfRange { brightness: 11 })
    );
    assert_eq!(
        brightness_compare_value(u8::MAX),
        Err(backlight_plan::BrightnessOutOfRange {
            brightness: u8::MAX
        })
    );
}
