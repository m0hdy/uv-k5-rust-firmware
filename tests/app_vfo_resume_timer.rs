#[path = "../src/app_vfo_resume_timer.rs"]
mod app_vfo_resume_timer;

use app_vfo_resume_timer::{Context, Effects, State, advance};

fn context() -> Context {
    Context {
        ptt_pressed: false,
        fm_feature_enabled: true,
        fm_radio_mode: false,
        receiving: false,
    }
}

#[test]
fn ptt_holds_the_countdown() {
    let mut state = State {
        resume_countdown: 1,
    };
    let effects = advance(
        &mut state,
        Context {
            ptt_pressed: true,
            ..context()
        },
    );
    assert_eq!(state.resume_countdown, 1);
    assert_eq!(effects, Effects::default());
}

#[test]
fn zero_countdown_does_not_trigger_expiry_again() {
    let mut state = State::default();
    assert_eq!(advance(&mut state, context()), Effects::default());
}

#[test]
fn positive_countdown_decrements_without_expiring() {
    let mut state = State {
        resume_countdown: 2,
    };
    assert_eq!(advance(&mut state, context()), Effects::default());
    assert_eq!(state.resume_countdown, 1);
}

#[test]
fn one_countdown_requests_normal_vfo_state() {
    let mut state = State {
        resume_countdown: 1,
    };
    assert_eq!(
        advance(&mut state, context()),
        Effects {
            restore_normal_vfo_state: true,
            restart_fm: false,
            select_fm_display: false,
        }
    );
    assert_eq!(state.resume_countdown, 0);
}

#[test]
fn fm_mode_restarts_and_selects_display_when_not_receiving() {
    let mut state = State {
        resume_countdown: 1,
    };
    let effects = advance(
        &mut state,
        Context {
            fm_radio_mode: true,
            ..context()
        },
    );
    assert_eq!(
        effects,
        Effects {
            restore_normal_vfo_state: true,
            restart_fm: true,
            select_fm_display: true,
        }
    );
}

#[test]
fn fm_mode_while_receiving_only_restores_normal_vfo_state() {
    let mut state = State {
        resume_countdown: 1,
    };
    let effects = advance(
        &mut state,
        Context {
            fm_radio_mode: true,
            receiving: true,
            ..context()
        },
    );
    assert_eq!(
        effects,
        Effects {
            restore_normal_vfo_state: true,
            restart_fm: false,
            select_fm_display: false,
        }
    );
}

#[test]
fn fm_feature_disabled_suppresses_fm_actions() {
    let mut state = State {
        resume_countdown: 1,
    };
    let effects = advance(
        &mut state,
        Context {
            fm_feature_enabled: false,
            fm_radio_mode: true,
            ..context()
        },
    );
    assert!(effects.restore_normal_vfo_state);
    assert!(!effects.restart_fm);
    assert!(!effects.select_fm_display);
}

#[test]
fn maximum_width_countdown_decrements_without_overflow() {
    let mut state = State {
        resume_countdown: u8::MAX,
    };
    assert_eq!(advance(&mut state, context()), Effects::default());
    assert_eq!(state.resume_countdown, u8::MAX - 1);
}
