#[path = "../src/app_500ms_service_timers.rs"]
mod app_500ms_service_timers;

use app_500ms_service_timers::{
    BACKLIGHT_ALWAYS_ON_INDEX, BacklightMenuItem, Context, Effects, State, advance,
};

fn context() -> Context {
    Context {
        enable_fmradio: false,
        fm_radio_mode: false,
        ask_to_save: false,
        css_background_scan: false,
        screen_is_menu: false,
        menu_item: BacklightMenuItem::Other,
        backlight_time: 3,
    }
}

fn state() -> State {
    State {
        fm_radio_countdown: 0,
        backlight_countdown: 0,
    }
}

#[test]
fn zero_counters_do_not_create_effects() {
    let mut value = state();
    let before = value;
    assert_eq!(advance(&mut value, context()), Effects::default());
    assert_eq!(value, before);
}

#[test]
fn disabled_fm_feature_preserves_its_countdown_and_continues() {
    let mut value = State {
        fm_radio_countdown: 2,
        backlight_countdown: 1,
    };
    let effects = advance(
        &mut value,
        Context {
            fm_radio_mode: true,
            ..context()
        },
    );
    assert_eq!(value.fm_radio_countdown, 2);
    assert_eq!(value.backlight_countdown, 0);
    assert_eq!(
        effects,
        Effects {
            return_from_timeslice: false,
            turn_backlight_off: true
        }
    );
}

#[test]
fn enabled_fm_mode_returns_early_after_decrement_even_at_zero() {
    for initial in [1, 2] {
        let mut value = State {
            fm_radio_countdown: initial,
            backlight_countdown: 1,
        };
        let effects = advance(
            &mut value,
            Context {
                enable_fmradio: true,
                fm_radio_mode: true,
                ..context()
            },
        );
        assert_eq!(value.fm_radio_countdown, initial - 1);
        assert_eq!(value.backlight_countdown, 1);
        assert_eq!(
            effects,
            Effects {
                return_from_timeslice: true,
                turn_backlight_off: false,
            }
        );
    }
}

#[test]
fn enabled_fm_countdown_decrements_without_early_return_outside_fm_mode() {
    let mut value = State {
        fm_radio_countdown: 1,
        ..state()
    };
    let effects = advance(
        &mut value,
        Context {
            enable_fmradio: true,
            ..context()
        },
    );
    assert_eq!(value.fm_radio_countdown, 0);
    assert_eq!(effects, Effects::default());
}

#[test]
fn zero_fm_countdown_does_not_early_return_in_fm_mode() {
    let mut value = State {
        backlight_countdown: 1,
        ..state()
    };
    let effects = advance(
        &mut value,
        Context {
            enable_fmradio: true,
            fm_radio_mode: true,
            ..context()
        },
    );
    assert_eq!(value.backlight_countdown, 0);
    assert!(effects.turn_backlight_off);
    assert!(!effects.return_from_timeslice);
}

#[test]
fn save_prompt_css_scan_and_backlight_menus_each_hold_the_countdown() {
    let contexts = [
        Context {
            ask_to_save: true,
            ..context()
        },
        Context {
            css_background_scan: true,
            ..context()
        },
        Context {
            screen_is_menu: true,
            menu_item: BacklightMenuItem::AutoBrightness,
            ..context()
        },
        Context {
            screen_is_menu: true,
            menu_item: BacklightMenuItem::MaximumBrightness,
            ..context()
        },
    ];
    for selected in contexts {
        let mut value = State {
            backlight_countdown: 1,
            ..state()
        };
        assert_eq!(advance(&mut value, selected), Effects::default());
        assert_eq!(value.backlight_countdown, 1);
    }
}

#[test]
fn other_menu_items_do_not_hold_backlight_countdown() {
    for menu_item in [
        BacklightMenuItem::Other,
        BacklightMenuItem::MinimumBrightness,
    ] {
        let mut value = State {
            backlight_countdown: 1,
            ..state()
        };
        let effects = advance(
            &mut value,
            Context {
                screen_is_menu: true,
                menu_item,
                ..context()
            },
        );
        assert_eq!(value.backlight_countdown, 0);
        assert!(effects.turn_backlight_off);
    }
}

#[test]
fn always_on_setting_suppresses_off_request_after_countdown_decrements() {
    let mut value = State {
        backlight_countdown: 1,
        ..state()
    };
    let effects = advance(
        &mut value,
        Context {
            backlight_time: BACKLIGHT_ALWAYS_ON_INDEX,
            ..context()
        },
    );
    assert_eq!(value.backlight_countdown, 0);
    assert_eq!(effects, Effects::default());
}

#[test]
fn timed_setting_requests_backlight_off_at_expiry() {
    let mut value = State {
        backlight_countdown: 1,
        ..state()
    };
    assert_eq!(
        advance(&mut value, context()),
        Effects {
            return_from_timeslice: false,
            turn_backlight_off: true,
        }
    );
    assert_eq!(value.backlight_countdown, 0);
}

#[test]
fn maximum_counters_decrement_without_wrapping() {
    let mut value = State {
        fm_radio_countdown: u8::MAX,
        backlight_countdown: u16::MAX,
    };
    advance(
        &mut value,
        Context {
            enable_fmradio: true,
            ..context()
        },
    );
    assert_eq!(value.fm_radio_countdown, u8::MAX - 1);
    assert_eq!(value.backlight_countdown, u16::MAX - 1);
}

#[test]
fn fm_early_return_prevents_simultaneous_backlight_expiry_effect() {
    let mut value = State {
        fm_radio_countdown: 1,
        backlight_countdown: 1,
    };
    let effects = advance(
        &mut value,
        Context {
            enable_fmradio: true,
            fm_radio_mode: true,
            ..context()
        },
    );
    assert_eq!(value.fm_radio_countdown, 0);
    assert_eq!(value.backlight_countdown, 1);
    assert!(effects.return_from_timeslice);
    assert!(!effects.turn_backlight_off);
}
