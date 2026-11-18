#[path = "../src/scan_decisions.rs"]
mod scan_decisions;

use scan_decisions::{
    FunctionState, ReceptionState, ResumeMode, ScanAction, ScanEvent, ScanKind, ScanState,
    StopDisposition,
};

fn start(kind: ScanKind, resume_mode: ResumeMode) -> ScanState {
    let mut state = ScanState::new();
    assert_eq!(
        state.apply(ScanEvent::Start {
            kind,
            resume_mode,
            store_backup_settings: true,
        }),
        ScanAction::Advance
    );
    state
}

fn ticks(state: &mut ScanState, count: u16, function: FunctionState) {
    for _ in 0..count {
        state.apply(ScanEvent::Tick { function });
    }
}

#[test]
fn start_initializes_both_scan_kinds_and_all_resume_modes() {
    let cases = [
        (ScanKind::Frequency, ResumeMode::Time),
        (ScanKind::Memory, ResumeMode::Time),
        (ScanKind::Frequency, ResumeMode::Carrier),
        (ScanKind::Memory, ResumeMode::Carrier),
        (ScanKind::Frequency, ResumeMode::Search),
        (ScanKind::Memory, ResumeMode::Search),
    ];

    for (kind, resume_mode) in cases {
        let state = start(kind, resume_mode);
        assert!(state.is_active());
        assert!(!state.is_paused());
        assert!(!state.is_scheduled());
        assert_eq!(state.remaining_10ms(), ScanState::START_DELAY_10MS);
        assert!(!state.keeps_last_result());
    }
}

#[test]
fn start_advances_immediately_before_the_initial_scan_delay_for_both_kinds() {
    for kind in [ScanKind::Frequency, ScanKind::Memory] {
        let mut state = ScanState::new();
        assert_eq!(
            state.apply(ScanEvent::Start {
                kind,
                resume_mode: ResumeMode::Time,
                store_backup_settings: true,
            }),
            ScanAction::Advance
        );
        assert_eq!(state.remaining_10ms(), 50);
        assert!(!state.is_scheduled());
    }
}

#[test]
fn timer_schedules_only_on_the_exact_expiry_tick() {
    let mut state = start(ScanKind::Frequency, ResumeMode::Time);

    ticks(
        &mut state,
        ScanState::START_DELAY_10MS - 1,
        FunctionState::Other,
    );
    assert_eq!(state.remaining_10ms(), 1);
    assert!(!state.is_scheduled());

    state.apply(ScanEvent::Tick {
        function: FunctionState::Other,
    });
    assert_eq!(state.remaining_10ms(), 0);
    assert!(state.is_scheduled());

    state.apply(ScanEvent::Tick {
        function: FunctionState::Other,
    });
    assert!(state.is_scheduled());
}

#[test]
fn monitor_and_transmit_ticks_do_not_consume_scan_delay() {
    for function in [FunctionState::Monitor, FunctionState::Transmit] {
        let mut state = start(ScanKind::Memory, ResumeMode::Carrier);
        ticks(&mut state, ScanState::START_DELAY_10MS, function);
        assert_eq!(state.remaining_10ms(), ScanState::START_DELAY_10MS);
        assert!(!state.is_scheduled());

        state.apply(ScanEvent::Tick {
            function: FunctionState::Incoming,
        });
        assert_eq!(state.remaining_10ms(), ScanState::START_DELAY_10MS - 1);
    }
}

#[test]
fn time_resume_pauses_once_and_repeated_found_events_do_not_restart_timer() {
    let mut state = start(ScanKind::Memory, ResumeMode::Time);
    state.apply(ScanEvent::Found);
    assert!(state.is_paused());
    assert!(state.keeps_last_result());
    assert_eq!(state.remaining_10ms(), ScanState::TIME_RESUME_DELAY_10MS);
    assert!(!state.is_scheduled());

    ticks(&mut state, 1, FunctionState::Incoming);
    state.apply(ScanEvent::Found);
    assert_eq!(
        state.remaining_10ms(),
        ScanState::TIME_RESUME_DELAY_10MS - 1
    );
    assert!(state.is_paused());
}

#[test]
fn time_resume_schedules_on_expiry_and_found_does_not_clear_it() {
    let mut state = start(ScanKind::Frequency, ResumeMode::Time);
    state.apply(ScanEvent::Found);
    ticks(
        &mut state,
        ScanState::TIME_RESUME_DELAY_10MS - 1,
        FunctionState::Incoming,
    );
    assert_eq!(state.remaining_10ms(), 1);
    assert!(!state.is_scheduled());

    state.apply(ScanEvent::Tick {
        function: FunctionState::Incoming,
    });
    assert_eq!(state.remaining_10ms(), 0);
    assert!(state.is_scheduled());

    state.apply(ScanEvent::Found);
    assert!(state.is_scheduled());
    assert_eq!(state.remaining_10ms(), 0);
}

#[test]
fn first_detection_restarts_delay_and_clears_a_pending_continuation() {
    let mut state = start(ScanKind::Memory, ResumeMode::Time);
    ticks(
        &mut state,
        ScanState::START_DELAY_10MS,
        FunctionState::Other,
    );
    assert!(state.is_scheduled());

    state.apply(ScanEvent::Detected);
    assert_eq!(state.remaining_10ms(), 20);
    assert!(!state.is_scheduled());
    assert_eq!(state.reception_state(), ReceptionState::Detected);
    assert!(state.is_detection_latched());

    ticks(&mut state, 5, FunctionState::Incoming);
    state.apply(ScanEvent::Detected);
    assert_eq!(state.remaining_10ms(), 15);
    assert!(!state.is_scheduled());
    assert_eq!(state.reception_state(), ReceptionState::Detected);

    state.apply(ScanEvent::Found);
    assert_eq!(state.reception_state(), ReceptionState::Listening);
    assert!(state.is_detection_latched());
    state.apply(ScanEvent::ReceptionEnded);
    assert_eq!(state.reception_state(), ReceptionState::None);
    assert!(state.is_detection_latched());
}

#[test]
fn carrier_resume_waits_until_reception_ends_then_schedules_at_360_ticks() {
    let mut state = start(ScanKind::Frequency, ResumeMode::Carrier);
    state.apply(ScanEvent::Found);
    assert_eq!(state.remaining_10ms(), 0);
    assert!(!state.is_scheduled());

    state.apply(ScanEvent::ReceptionEnded);
    ticks(
        &mut state,
        ScanState::CARRIER_RESUME_DELAY_10MS - 1,
        FunctionState::Incoming,
    );
    assert_eq!(state.remaining_10ms(), 1);
    assert!(!state.is_scheduled());

    state.apply(ScanEvent::Tick {
        function: FunctionState::Other,
    });
    assert_eq!(state.remaining_10ms(), 0);
    assert!(state.is_scheduled());
}

#[test]
fn carrier_found_before_resume_expiry_restarts_delay_after_reception_ends() {
    let mut state = start(ScanKind::Memory, ResumeMode::Carrier);
    state.apply(ScanEvent::Found);
    state.apply(ScanEvent::ReceptionEnded);
    ticks(&mut state, 1, FunctionState::Incoming);

    state.apply(ScanEvent::Found);
    assert_eq!(state.remaining_10ms(), 0);
    assert!(!state.is_scheduled());

    state.apply(ScanEvent::ReceptionEnded);
    assert_eq!(state.remaining_10ms(), ScanState::CARRIER_RESUME_DELAY_10MS);
    assert!(!state.is_scheduled());
}

#[test]
fn returning_detection_does_not_replace_a_resume_timer_while_latched() {
    let cases = [
        (ResumeMode::Time, ScanState::TIME_RESUME_DELAY_10MS),
        (ResumeMode::Carrier, ScanState::CARRIER_RESUME_DELAY_10MS),
    ];

    for (resume_mode, expected_delay) in cases {
        let mut state = start(ScanKind::Memory, resume_mode);
        state.apply(ScanEvent::Detected);
        state.apply(ScanEvent::Found);
        state.apply(ScanEvent::ReceptionEnded);
        assert_eq!(state.remaining_10ms(), expected_delay);
        assert_eq!(state.reception_state(), ReceptionState::None);
        assert!(state.is_detection_latched());

        state.apply(ScanEvent::Detected);
        assert_eq!(state.remaining_10ms(), expected_delay);
        assert!(state.is_detection_latched());
    }
}

#[test]
fn listen_continuation_clears_the_detection_latch_for_a_fresh_detection() {
    for resume_mode in [ResumeMode::Time, ResumeMode::Carrier] {
        let mut state = start(ScanKind::Memory, resume_mode);
        ticks(
            &mut state,
            ScanState::START_DELAY_10MS,
            FunctionState::Other,
        );
        assert_eq!(
            state.apply(ScanEvent::Continue {
                function: FunctionState::Incoming,
                code_type_off: true,
            }),
            ScanAction::Listen
        );
        assert!(state.keeps_last_result());
        assert_eq!(state.reception_state(), ReceptionState::Listening);
        assert!(!state.is_detection_latched());

        state.apply(ScanEvent::ReceptionEnded);
        assert_eq!(state.reception_state(), ReceptionState::None);
        state.apply(ScanEvent::Detected);
        assert_eq!(state.remaining_10ms(), ScanState::DETECTION_DELAY_10MS);
        assert!(state.is_detection_latched());
    }
}

#[test]
fn time_resume_redetection_restarts_the_pending_timer_after_listen() {
    let mut state = start(ScanKind::Memory, ResumeMode::Time);
    state.apply(ScanEvent::Detected);
    ticks(&mut state, 20, FunctionState::Incoming);
    assert_eq!(
        state.apply(ScanEvent::Continue {
            function: FunctionState::Incoming,
            code_type_off: true,
        }),
        ScanAction::Listen
    );
    assert_eq!(state.remaining_10ms(), 500);
    assert!(state.keeps_last_result());

    state.apply(ScanEvent::ReceptionEnded);
    ticks(&mut state, 100, FunctionState::Incoming);
    assert_eq!(state.remaining_10ms(), 400);

    state.apply(ScanEvent::Detected);
    assert_eq!(state.remaining_10ms(), 20);
    assert!(!state.is_scheduled());
    ticks(&mut state, 20, FunctionState::Incoming);
    assert!(state.is_scheduled());
    assert_eq!(
        state.apply(ScanEvent::Continue {
            function: FunctionState::Incoming,
            code_type_off: true,
        }),
        ScanAction::Listen
    );
    assert_eq!(state.remaining_10ms(), 500);
    assert!(state.keeps_last_result());

    state.apply(ScanEvent::ReceptionEnded);
    assert_eq!(state.remaining_10ms(), 500);
    assert!(!state.is_scheduled());
}

#[test]
fn carrier_resume_redetection_replaces_the_pending_timer_and_restarts_after_end() {
    let mut state = start(ScanKind::Frequency, ResumeMode::Carrier);
    state.apply(ScanEvent::Detected);
    ticks(&mut state, 20, FunctionState::Incoming);
    assert_eq!(
        state.apply(ScanEvent::Continue {
            function: FunctionState::Incoming,
            code_type_off: false,
        }),
        ScanAction::Listen
    );
    assert_eq!(state.remaining_10ms(), 0);

    state.apply(ScanEvent::ReceptionEnded);
    assert_eq!(state.remaining_10ms(), 360);
    ticks(&mut state, 10, FunctionState::Incoming);
    assert_eq!(state.remaining_10ms(), 350);

    state.apply(ScanEvent::Detected);
    assert_eq!(state.remaining_10ms(), 20);
    assert!(!state.is_scheduled());
    ticks(&mut state, 20, FunctionState::Incoming);
    assert!(state.is_scheduled());
    assert_eq!(
        state.apply(ScanEvent::Continue {
            function: FunctionState::Incoming,
            code_type_off: false,
        }),
        ScanAction::Listen
    );
    assert_eq!(state.remaining_10ms(), 0);
    assert!(state.keeps_last_result());

    state.apply(ScanEvent::ReceptionEnded);
    assert_eq!(state.remaining_10ms(), 360);
    assert!(!state.is_scheduled());
}

#[test]
fn search_resume_stops_after_reception_ends_and_retains_a_found_result() {
    let mut state = start(ScanKind::Memory, ResumeMode::Search);
    state.apply(ScanEvent::Found);
    assert_eq!(
        state.apply(ScanEvent::ReceptionEnded),
        ScanAction::Stopped(StopDisposition::KeepLastFound)
    );
    assert!(!state.is_active());
    assert!(!state.is_scheduled());
}

#[test]
fn continue_decisions_match_scan_kind_and_incoming_code_state() {
    let cases = [
        (
            ScanKind::Frequency,
            FunctionState::Incoming,
            false,
            ScanAction::Listen,
        ),
        (
            ScanKind::Frequency,
            FunctionState::Incoming,
            true,
            ScanAction::Listen,
        ),
        (
            ScanKind::Memory,
            FunctionState::Incoming,
            true,
            ScanAction::Listen,
        ),
        (
            ScanKind::Memory,
            FunctionState::Incoming,
            false,
            ScanAction::Advance,
        ),
        (
            ScanKind::Frequency,
            FunctionState::Other,
            true,
            ScanAction::Advance,
        ),
        (
            ScanKind::Memory,
            FunctionState::Other,
            false,
            ScanAction::Advance,
        ),
    ];

    for (kind, function, code_type_off, expected) in cases {
        let mut state = start(kind, ResumeMode::Time);
        ticks(
            &mut state,
            ScanState::START_DELAY_10MS,
            FunctionState::Other,
        );
        assert_eq!(
            state.apply(ScanEvent::Continue {
                function,
                code_type_off,
            }),
            expected
        );
        assert!(!state.is_scheduled());
        assert!(!state.is_paused());
        assert_eq!(
            state.reception_state(),
            if expected == ScanAction::Listen {
                ReceptionState::Listening
            } else {
                ReceptionState::None
            }
        );
        assert!(!state.is_detection_latched());
        assert_eq!(
            state.remaining_10ms(),
            if expected == ScanAction::Advance {
                ScanState::NEXT_CHANNEL_DELAY_10MS
            } else if expected == ScanAction::Listen {
                500
            } else {
                0
            }
        );
        if expected == ScanAction::Listen {
            assert!(state.keeps_last_result());
        }
    }
}

#[test]
fn listen_continuation_applies_found_before_clearing_pause_and_keeps_result() {
    let mut state = start(ScanKind::Frequency, ResumeMode::Time);
    ticks(
        &mut state,
        ScanState::START_DELAY_10MS,
        FunctionState::Other,
    );

    assert_eq!(
        state.apply(ScanEvent::Continue {
            function: FunctionState::Incoming,
            code_type_off: false,
        }),
        ScanAction::Listen
    );
    assert_eq!(state.remaining_10ms(), 500);
    assert!(state.keeps_last_result());
    assert_eq!(state.reception_state(), ReceptionState::Listening);
    assert!(!state.is_detection_latched());
    assert!(state.is_active());
    assert_eq!(
        state.apply(ScanEvent::Stop),
        ScanAction::Stopped(StopDisposition::KeepLastFound)
    );
}

#[test]
fn advance_clears_detection_state_for_a_fresh_next_channel_window() {
    for resume_mode in [ResumeMode::Time, ResumeMode::Carrier] {
        let mut state = start(ScanKind::Memory, resume_mode);
        ticks(
            &mut state,
            ScanState::START_DELAY_10MS,
            FunctionState::Other,
        );
        state.apply(ScanEvent::Detected);
        ticks(
            &mut state,
            ScanState::DETECTION_DELAY_10MS,
            FunctionState::Incoming,
        );
        assert!(state.is_scheduled());

        assert_eq!(
            state.apply(ScanEvent::Continue {
                function: FunctionState::Incoming,
                code_type_off: false,
            }),
            ScanAction::Advance
        );
        assert_eq!(state.reception_state(), ReceptionState::None);
        assert!(!state.is_detection_latched());
        assert_eq!(state.remaining_10ms(), ScanState::NEXT_CHANNEL_DELAY_10MS);

        state.apply(ScanEvent::Detected);
        assert_eq!(state.reception_state(), ReceptionState::Detected);
        assert!(state.is_detection_latched());
        assert_eq!(state.remaining_10ms(), ScanState::DETECTION_DELAY_10MS);
        assert!(!state.is_scheduled());
    }
}

#[test]
fn channel_advance_uses_the_pinned_90ms_delay_for_both_scan_kinds() {
    for kind in [ScanKind::Frequency, ScanKind::Memory] {
        let mut state = start(kind, ResumeMode::Time);
        ticks(
            &mut state,
            ScanState::START_DELAY_10MS,
            FunctionState::Other,
        );
        assert_eq!(
            state.apply(ScanEvent::Continue {
                function: FunctionState::Other,
                code_type_off: false,
            }),
            ScanAction::Advance
        );

        assert_eq!(state.remaining_10ms(), 9);
        ticks(&mut state, 8, FunctionState::Other);
        assert_eq!(state.remaining_10ms(), 1);
        assert!(!state.is_scheduled());
        state.apply(ScanEvent::Tick {
            function: FunctionState::Other,
        });
        assert_eq!(state.remaining_10ms(), 0);
        assert!(state.is_scheduled());
    }
}

#[test]
fn continue_without_an_expired_schedule_does_not_change_scan_state() {
    let mut state = start(ScanKind::Frequency, ResumeMode::Time);
    assert_eq!(
        state.apply(ScanEvent::Continue {
            function: FunctionState::Other,
            code_type_off: true,
        }),
        ScanAction::None
    );
    assert_eq!(state.remaining_10ms(), ScanState::START_DELAY_10MS);
}

#[test]
fn stopping_selects_the_initial_or_last_found_position() {
    let mut not_found = start(ScanKind::Frequency, ResumeMode::Time);
    assert_eq!(
        not_found.apply(ScanEvent::Stop),
        ScanAction::Stopped(StopDisposition::RestoreInitial)
    );

    let mut found = start(ScanKind::Memory, ResumeMode::Carrier);
    found.apply(ScanEvent::Found);
    assert_eq!(
        found.apply(ScanEvent::Stop),
        ScanAction::Stopped(StopDisposition::KeepLastFound)
    );
}

#[test]
fn restarting_without_backup_preserves_the_prior_found_result() {
    let mut state = start(ScanKind::Memory, ResumeMode::Time);
    state.apply(ScanEvent::Found);
    state.apply(ScanEvent::Start {
        kind: ScanKind::Memory,
        resume_mode: ResumeMode::Search,
        store_backup_settings: false,
    });
    assert!(state.keeps_last_result());

    state.apply(ScanEvent::Start {
        kind: ScanKind::Memory,
        resume_mode: ResumeMode::Search,
        store_backup_settings: true,
    });
    assert!(!state.keeps_last_result());
}

#[test]
fn repeated_reception_end_and_events_after_stop_are_harmless() {
    let mut state = start(ScanKind::Memory, ResumeMode::Search);
    state.apply(ScanEvent::Found);
    assert_eq!(
        state.apply(ScanEvent::ReceptionEnded),
        ScanAction::Stopped(StopDisposition::KeepLastFound)
    );
    assert_eq!(state.apply(ScanEvent::ReceptionEnded), ScanAction::None);
    assert_eq!(state.apply(ScanEvent::Found), ScanAction::None);
    assert!(!state.is_active());
    assert!(!state.is_scheduled());
}
