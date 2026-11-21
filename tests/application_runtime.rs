use uv_k5_rust_firmware::app_500ms_service_timers::BacklightMenuItem;
use uv_k5_rust_firmware::app_500ms_timeouts::CenterLine;
use uv_k5_rust_firmware::application_runtime::{Context, State};
use uv_k5_rust_firmware::keyboard_poll_flow::{Key, KeyEvent};

fn context() -> Context {
    Context {
        enable_rssi_bar: false,
        center_line: CenterLine::None,
        enable_fmradio: false,
        fm_radio_mode: false,
        ask_to_save: false,
        css_background_scan: false,
        backlight_menu_item: BacklightMenuItem::Other,
        backlight_time: 3,
    }
}

#[test]
fn digits_enter_on_unheld_release_not_press_or_held_repeat() {
    let mut state = State::new();
    state.on_key_event(KeyEvent::Pressed(Key::Digit4), 60);
    assert_eq!(state.input_box().index(), 0);
    state.on_key_event(
        KeyEvent::Released {
            key: Key::Digit4,
            was_held: true,
        },
        60,
    );
    assert_eq!(state.input_box().index(), 0);
    state.on_key_event(
        KeyEvent::Released {
            key: Key::Digit4,
            was_held: false,
        },
        60,
    );
    state.on_key_event(
        KeyEvent::Released {
            key: Key::Digit2,
            was_held: false,
        },
        60,
    );

    assert_eq!(state.input_box().index(), 2);
    assert_eq!(&state.input_box().ascii_projection()[..2], b"42");
    assert_eq!(
        state.last_key_event(),
        Some(KeyEvent::Released {
            key: Key::Digit2,
            was_held: false
        })
    );
    assert_eq!(state.key_event_count(), 4);
}

#[test]
fn menu_and_exit_events_follow_the_application_screen_state() {
    let mut state = State::new();
    assert!(
        state
            .on_key_event(KeyEvent::Pressed(Key::Menu), 60)
            .enter_menu
    );
    assert!(state.screen_is_menu());

    assert!(
        state
            .on_key_event(KeyEvent::Pressed(Key::Exit), 60)
            .exit_menu
    );
    assert!(!state.screen_is_menu());
}

#[test]
fn menu_timeout_exits_menu_and_keeps_display_refresh_sticky() {
    let mut state = State::new();
    state.on_key_event(KeyEvent::Pressed(Key::Menu), 1);
    let effects = state.timeslice_500ms(context());

    assert!(effects.exit_menu);
    assert!(!state.screen_is_menu());
    assert!(state.update_display());
}

#[test]
fn fm_early_return_preserves_the_backlight_countdown() {
    let mut state = State::new();
    state.set_countdowns(0, 0, 0, 1, 1);
    let effects = state.timeslice_500ms(Context {
        enable_fmradio: true,
        fm_radio_mode: true,
        ..context()
    });

    assert!(effects.return_from_timeslice);
    assert!(!effects.turn_backlight_off);
}

#[test]
fn backlight_expiry_is_returned_as_an_application_effect() {
    let mut state = State::new();
    state.set_countdowns(0, 0, 0, 0, 1);

    let effects = state.timeslice_500ms(context());

    assert!(effects.turn_backlight_off);
}
