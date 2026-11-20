#[path = "../src/app_500ms_timeouts.rs"]
mod app_500ms_timeouts;

use app_500ms_timeouts::{CenterLine, Context, Effects, State, advance};

fn context(enable_rssi_bar: bool, center_line: CenterLine, screen_is_menu: bool) -> Context {
    Context {
        enable_rssi_bar,
        center_line,
        screen_is_menu,
    }
}

fn state() -> State {
    State {
        keypad_locked_countdown: 0,
        dtmf_live_timeout: 0,
        dtmf_live_text: [0; 20],
        menu_countdown: 0,
        update_display: false,
    }
}

#[test]
fn zero_counters_remain_zero_and_do_not_create_effects() {
    let mut value = state();
    let before = value;

    assert_eq!(
        advance(&mut value, context(true, CenterLine::InUse, true)),
        Effects::default()
    );
    assert_eq!(value, before);
}

#[test]
fn keypad_lock_expiry_requests_display_update_and_preserves_sticky_update() {
    let mut value = State {
        keypad_locked_countdown: 2,
        ..state()
    };
    assert_eq!(
        advance(&mut value, context(false, CenterLine::None, false)),
        Effects::default()
    );
    assert_eq!(value.keypad_locked_countdown, 1);
    assert!(!value.update_display);

    advance(&mut value, context(false, CenterLine::None, false));
    assert_eq!(value.keypad_locked_countdown, 0);
    assert!(value.update_display);

    value.keypad_locked_countdown = 1;
    advance(&mut value, context(false, CenterLine::None, false));
    assert!(value.update_display);
}

#[test]
fn dtmf_timeout_waits_until_rssi_center_line_is_available() {
    let mut value = State {
        dtmf_live_timeout: 2,
        dtmf_live_text: *b"1234567890123456789\0",
        ..state()
    };
    advance(&mut value, context(true, CenterLine::Rssi, false));
    assert_eq!(value.dtmf_live_timeout, 2);
    assert_eq!(&value.dtmf_live_text[..4], b"1234");

    advance(&mut value, context(true, CenterLine::DtmfDecode, false));
    assert_eq!(value.dtmf_live_timeout, 1);
    assert_ne!(value.dtmf_live_text[0], 0);
    advance(&mut value, context(true, CenterLine::None, false));
    assert_eq!(value.dtmf_live_timeout, 0);
    assert_eq!(value.dtmf_live_text, [0; 20]);
    assert!(value.update_display);
}

#[test]
fn every_other_rssi_center_line_delays_dtmf_expiry() {
    for center_line in [
        CenterLine::InUse,
        CenterLine::AudioBar,
        CenterLine::Rssi,
        CenterLine::AmFixData,
        CenterLine::ChargeData,
    ] {
        let mut value = State {
            dtmf_live_timeout: 1,
            dtmf_live_text: *b"A123456789012345678\0",
            ..state()
        };
        advance(&mut value, context(true, center_line, false));
        assert_eq!(value.dtmf_live_timeout, 1, "{center_line:?}");
        assert_ne!(value.dtmf_live_text[0], 0, "{center_line:?}");
        assert!(!value.update_display, "{center_line:?}");
    }
}

#[test]
fn dtmf_timeout_ignores_center_line_when_rssi_bar_is_disabled() {
    let mut value = State {
        dtmf_live_timeout: 1,
        dtmf_live_text: *b"A123456789012345678\0",
        ..state()
    };
    advance(&mut value, context(false, CenterLine::Rssi, false));
    assert_eq!(value.dtmf_live_timeout, 0);
    assert_eq!(value.dtmf_live_text, [0; 20]);
    assert!(value.update_display);
}

#[test]
fn dtmf_expiry_with_empty_text_does_not_request_display_update() {
    let mut value = State {
        dtmf_live_timeout: 1,
        ..state()
    };
    advance(&mut value, context(false, CenterLine::InUse, false));
    assert_eq!(value.dtmf_live_timeout, 0);
    assert!(!value.update_display);
}

#[test]
fn menu_expiry_requests_exit_only_when_menu_is_selected() {
    let mut value = State {
        menu_countdown: 1,
        ..state()
    };
    assert_eq!(
        advance(&mut value, context(false, CenterLine::None, false)),
        Effects { exit_menu: false }
    );
    assert_eq!(value.menu_countdown, 0);

    value.menu_countdown = 1;
    assert_eq!(
        advance(&mut value, context(false, CenterLine::None, true)),
        Effects { exit_menu: true }
    );
    assert_eq!(value.menu_countdown, 0);
}

#[test]
fn simultaneous_expiries_preserve_source_order_and_return_menu_effect() {
    let mut value = State {
        keypad_locked_countdown: 1,
        dtmf_live_timeout: 1,
        dtmf_live_text: *b"ABCDEFGHIJKLMNOPQRS\0",
        menu_countdown: 1,
        update_display: false,
    };
    assert_eq!(
        advance(&mut value, context(true, CenterLine::DtmfDecode, true)),
        Effects { exit_menu: true }
    );
    assert_eq!(value.keypad_locked_countdown, 0);
    assert_eq!(value.dtmf_live_timeout, 0);
    assert_eq!(value.dtmf_live_text, [0; 20]);
    assert_eq!(value.menu_countdown, 0);
    assert!(value.update_display);
}

#[test]
fn maximum_counter_widths_decrement_without_wrapping() {
    let mut value = State {
        keypad_locked_countdown: u8::MAX,
        dtmf_live_timeout: u8::MAX,
        dtmf_live_text: [0; 20],
        menu_countdown: u16::MAX,
        update_display: false,
    };
    advance(&mut value, context(true, CenterLine::None, true));
    assert_eq!(value.keypad_locked_countdown, u8::MAX - 1);
    assert_eq!(value.dtmf_live_timeout, u8::MAX - 1);
    assert_eq!(value.menu_countdown, u16::MAX - 1);
    assert!(!value.update_display);
}
