#[path = "../src/keyboard_decisions.rs"]
mod keyboard_decisions;

use keyboard_decisions::{
    DEBOUNCE_TICKS, INITIAL_REPEAT_TICKS, Key, KeyEvent, KeyboardDecisions, REPEAT_INTERVAL_TICKS,
    SAMPLE_PERIOD_MS,
};

fn press_key(decisions: &mut KeyboardDecisions, key: Key) {
    assert_eq!(decisions.sample(Some(key)), None);
    for _ in 1..DEBOUNCE_TICKS {
        assert_eq!(decisions.sample(Some(key)), None);
    }
    assert_eq!(decisions.sample(Some(key)), Some(KeyEvent::Pressed(key)));
}

#[test]
fn initial_press_is_emitted_after_two_unchanged_ticks() {
    let mut decisions = KeyboardDecisions::new();

    assert_eq!(decisions.sample(Some(Key::Menu)), None);
    assert_eq!(decisions.sample(Some(Key::Menu)), None);
    assert_eq!(
        decisions.sample(Some(Key::Menu)),
        Some(KeyEvent::Pressed(Key::Menu))
    );
}

#[test]
fn release_is_emitted_after_two_unchanged_invalid_ticks() {
    let mut decisions = KeyboardDecisions::new();
    press_key(&mut decisions, Key::Digit4);

    assert_eq!(decisions.sample(None), None);
    assert_eq!(decisions.sample(None), None);
    assert_eq!(
        decisions.sample(None),
        Some(KeyEvent::Released {
            key: Key::Digit4,
            was_held: false
        })
    );
}

#[test]
fn bouncing_samples_do_not_commit_a_press() {
    let mut decisions = KeyboardDecisions::new();

    assert_eq!(decisions.sample(Some(Key::Menu)), None);
    assert_eq!(decisions.sample(None), None);
    assert_eq!(decisions.sample(Some(Key::Down)), None);
    assert_eq!(decisions.sample(None), None);
    assert_eq!(decisions.sample(None), None);
    assert_eq!(decisions.sample(None), None);
}

#[test]
fn changing_keys_releases_the_old_key_immediately_and_debounces_the_new_one() {
    let mut decisions = KeyboardDecisions::new();
    press_key(&mut decisions, Key::Digit1);

    assert_eq!(
        decisions.sample(Some(Key::Digit2)),
        Some(KeyEvent::Released {
            key: Key::Digit1,
            was_held: false
        })
    );
    assert_eq!(decisions.sample(Some(Key::Digit2)), None);
    assert_eq!(
        decisions.sample(Some(Key::Digit2)),
        Some(KeyEvent::Pressed(Key::Digit2))
    );
}

#[test]
fn repeat_thresholds_are_exact_and_only_up_down_repeat_quickly() {
    assert_eq!(SAMPLE_PERIOD_MS, 10);
    assert_eq!(DEBOUNCE_TICKS, 2);
    assert_eq!(INITIAL_REPEAT_TICKS, 40);
    assert_eq!(REPEAT_INTERVAL_TICKS, 8);

    let mut decisions = KeyboardDecisions::new();
    assert_eq!(decisions.sample(Some(Key::Up)), None);
    assert_eq!(decisions.sample(Some(Key::Up)), None);
    assert_eq!(
        decisions.sample(Some(Key::Up)),
        Some(KeyEvent::Pressed(Key::Up))
    );

    for _ in 0..37 {
        assert_eq!(decisions.sample(Some(Key::Up)), None);
    }
    assert_eq!(
        decisions.sample(Some(Key::Up)),
        Some(KeyEvent::Held(Key::Up))
    );

    for _ in 0..7 {
        assert_eq!(decisions.sample(Some(Key::Up)), None);
    }
    assert_eq!(
        decisions.sample(Some(Key::Up)),
        Some(KeyEvent::Held(Key::Up))
    );
    for _ in 0..7 {
        assert_eq!(decisions.sample(Some(Key::Up)), None);
    }
    assert_eq!(
        decisions.sample(Some(Key::Up)),
        Some(KeyEvent::Held(Key::Up))
    );

    let mut menu = KeyboardDecisions::new();
    press_key(&mut menu, Key::Menu);
    for _ in 0..37 {
        assert_eq!(menu.sample(Some(Key::Menu)), None);
    }
    assert_eq!(
        menu.sample(Some(Key::Menu)),
        Some(KeyEvent::Held(Key::Menu))
    );
    for _ in 0..8 {
        assert_eq!(menu.sample(Some(Key::Menu)), None);
    }
    assert_eq!(
        menu.sample(Some(Key::Menu)),
        None,
        "non-directional keys do not repeat after their initial held event"
    );
}

#[test]
fn ptt_does_not_emit_a_repeat_event() {
    let mut decisions = KeyboardDecisions::new();
    press_key(&mut decisions, Key::Ptt);

    for _ in DEBOUNCE_TICKS..INITIAL_REPEAT_TICKS - 1 {
        assert_eq!(decisions.sample(Some(Key::Ptt)), None);
    }
    for _ in 0..=REPEAT_INTERVAL_TICKS {
        assert_eq!(decisions.sample(Some(Key::Ptt)), None);
    }
}

#[test]
fn release_reports_whether_the_key_had_been_held() {
    let mut decisions = KeyboardDecisions::new();
    press_key(&mut decisions, Key::Down);
    for _ in DEBOUNCE_TICKS..INITIAL_REPEAT_TICKS - 1 {
        assert_eq!(decisions.sample(Some(Key::Down)), None);
    }
    assert_eq!(
        decisions.sample(Some(Key::Down)),
        Some(KeyEvent::Held(Key::Down))
    );

    assert_eq!(decisions.sample(None), None);
    assert_eq!(decisions.sample(None), None);
    assert_eq!(
        decisions.sample(None),
        Some(KeyEvent::Released {
            key: Key::Down,
            was_held: true
        })
    );
}
