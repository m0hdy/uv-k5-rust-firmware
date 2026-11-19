#[path = "../src/battery_status_text.rs"]
mod battery_status_text;

use battery_status_text::battery_status_text;

fn text(mode: u8, voltage: u16, percentage: u8) -> Option<String> {
    battery_status_text(mode, voltage, percentage, 0, false)
        .map(|status| String::from_utf8(status.as_bytes().to_vec()).unwrap())
}

#[test]
fn formats_voltage_with_two_fractional_digits_and_clamps_to_999_hundredths() {
    assert_eq!(text(1, 0, 0).as_deref(), Some("0.00V"));
    assert_eq!(text(1, 102, 0).as_deref(), Some("1.02V"));
    assert_eq!(text(1, 999, 0).as_deref(), Some("9.99V"));
    assert_eq!(text(1, 1000, 0).as_deref(), Some("9.99V"));
    assert_eq!(text(1, u16::MAX, 0).as_deref(), Some("9.99V"));
}

#[test]
fn formats_percentage_without_leading_zeroes() {
    for (percentage, expected) in [
        (0, "0%"),
        (9, "9%"),
        (10, "10%"),
        (99, "99%"),
        (100, "100%"),
    ] {
        assert_eq!(
            text(2, 0, percentage).as_deref(),
            Some(expected),
            "{percentage}%"
        );
    }
}

#[test]
fn disabled_and_unknown_modes_have_no_visible_text() {
    for mode in [0, 3, u8::MAX] {
        assert_eq!(battery_status_text(mode, 999, 100, 0, false), None);
    }
}

#[test]
fn right_aligns_with_conditional_usb_reservation() {
    let not_charging = battery_status_text(2, 0, 0, 0, false).unwrap();
    assert_eq!(not_charging.as_bytes(), b"0%");
    assert_eq!(not_charging.start_column, 97);

    let charging = battery_status_text(2, 0, 0, 0, true).unwrap();
    assert_eq!(charging.as_bytes(), b"0%");
    assert_eq!(charging.start_column, 88);
}

#[test]
fn allows_exact_fit_and_rejects_a_one_pixel_overlap() {
    let exact_fit = battery_status_text(1, 0, 0, 76, false).unwrap();
    assert_eq!(exact_fit.as_bytes(), b"0.00V");
    assert_eq!(exact_fit.start_column, 76);

    assert_eq!(battery_status_text(1, 0, 0, 77, false), None);
}

#[test]
fn oversized_occupied_prefix_is_rejected_without_overflow() {
    assert_eq!(battery_status_text(2, 0, 100, usize::MAX, false), None);
}
