#[path = "../src/app_scan_flow.rs"]
mod app_scan_flow;

use app_scan_flow::{
    AppScanFlow, ContinuationConditions, ContinuationResult, FunctionState, ListenMode,
    ReceptionState, ResumeMode, ScanAction, ScanEffect, ScanEffects, ScanKind, ScanState,
    StopDisposition,
};

#[derive(Default)]
struct Effects(Vec<ScanEffect>);

impl ScanEffects for Effects {
    fn emit(&mut self, effect: ScanEffect) {
        self.0.push(effect);
    }
}

fn scheduled_flow(kind: ScanKind) -> (AppScanFlow, Effects) {
    let mut flow = AppScanFlow::new();
    let mut effects = Effects::default();
    flow.start(kind, ResumeMode::Time, true, &mut effects);
    for _ in 0..app_scan_flow::ScanState::START_DELAY_10MS {
        flow.tick(FunctionState::Other);
    }
    assert!(flow.state().is_scheduled());
    (flow, effects)
}

#[test]
fn start_retains_kind_and_emits_the_initial_advance() {
    for (kind, expected) in [
        (ScanKind::Frequency, ScanEffect::AdvanceFrequency),
        (ScanKind::Memory, ScanEffect::AdvanceMemory),
    ] {
        let mut flow = AppScanFlow::new();
        let mut effects = Effects::default();
        flow.start(kind, ResumeMode::Time, true, &mut effects);

        assert_eq!(flow.kind(), Some(kind));
        assert_eq!(flow.state().remaining_10ms(), 50);
        assert_eq!(effects.0, [expected]);
    }
}

#[test]
fn unscheduled_and_inactive_scans_do_not_emit_effects() {
    let mut flow = AppScanFlow::new();
    let mut effects = Effects::default();
    assert_eq!(
        flow.continue_if_ready(ContinuationConditions::default(), &mut effects),
        ContinuationResult::NotScheduled
    );
    assert!(effects.0.is_empty());

    flow.start(ScanKind::Frequency, ResumeMode::Time, true, &mut effects);
    effects.0.clear();
    assert_eq!(
        flow.continue_if_ready(ContinuationConditions::default(), &mut effects),
        ContinuationResult::NotScheduled
    );
    assert!(effects.0.is_empty());
}

#[test]
fn each_application_blocker_preserves_the_pending_continuation() {
    let blockers = [
        ContinuationConditions {
            reduced_service: true,
            ..ContinuationConditions::default()
        },
        ContinuationConditions {
            fm_radio_mode: true,
            fm_countdown_pending: true,
            ..ContinuationConditions::default()
        },
        ContinuationConditions {
            background_scanner_active: true,
            ..ContinuationConditions::default()
        },
        ContinuationConditions {
            ptt_pressed: true,
            ..ContinuationConditions::default()
        },
        ContinuationConditions {
            voice_enabled: true,
            voice_queue_empty: false,
            ..ContinuationConditions::default()
        },
    ];

    for conditions in blockers {
        let (mut flow, mut effects) = scheduled_flow(ScanKind::Memory);
        effects.0.clear();
        assert_eq!(
            flow.continue_if_ready(conditions, &mut effects),
            ContinuationResult::Blocked
        );
        assert!(flow.state().is_scheduled());
        assert_eq!(effects.0, []);

        assert_eq!(
            flow.continue_if_ready(ContinuationConditions::default(), &mut effects),
            ContinuationResult::Emitted(ScanEffect::AdvanceMemory)
        );
        assert!(!flow.state().is_scheduled());
        assert_eq!(effects.0, [ScanEffect::AdvanceMemory]);
    }
}

#[test]
fn fm_countdown_only_blocks_while_fm_mode_is_active() {
    let (mut flow, mut effects) = scheduled_flow(ScanKind::Frequency);
    effects.0.clear();
    let conditions = ContinuationConditions {
        fm_countdown_pending: true,
        ..ContinuationConditions::default()
    };
    assert_eq!(
        flow.continue_if_ready(conditions, &mut effects),
        ContinuationResult::Emitted(ScanEffect::AdvanceFrequency)
    );
}

#[test]
fn frequency_incoming_listens_regardless_of_code_type_and_monitor_mode() {
    for (monitor, expected) in [(false, ListenMode::Receive), (true, ListenMode::Monitor)] {
        let (mut flow, mut effects) = scheduled_flow(ScanKind::Frequency);
        effects.0.clear();
        let conditions = ContinuationConditions {
            function: FunctionState::Incoming,
            code_type_off: false,
            monitor,
            ..ContinuationConditions::default()
        };
        let effect = ScanEffect::Listen(expected);
        assert_eq!(
            flow.continue_if_ready(conditions, &mut effects),
            ContinuationResult::Emitted(effect)
        );
        assert_eq!(effects.0, [effect]);
    }
}

#[test]
fn memory_incoming_listens_only_when_code_type_is_off() {
    let (mut listen_flow, mut listen_effects) = scheduled_flow(ScanKind::Memory);
    listen_effects.0.clear();
    assert_eq!(
        listen_flow.continue_if_ready(
            ContinuationConditions {
                function: FunctionState::Incoming,
                code_type_off: true,
                ..ContinuationConditions::default()
            },
            &mut listen_effects
        ),
        ContinuationResult::Emitted(ScanEffect::Listen(ListenMode::Receive))
    );

    let (mut advance_flow, mut advance_effects) = scheduled_flow(ScanKind::Memory);
    advance_effects.0.clear();
    assert_eq!(
        advance_flow.continue_if_ready(
            ContinuationConditions {
                function: FunctionState::Incoming,
                code_type_off: false,
                ..ContinuationConditions::default()
            },
            &mut advance_effects
        ),
        ContinuationResult::Emitted(ScanEffect::AdvanceMemory)
    );
}

#[test]
fn monitor_and_transmit_are_not_continuation_blockers() {
    for function in [FunctionState::Monitor, FunctionState::Transmit] {
        let (mut flow, mut effects) = scheduled_flow(ScanKind::Memory);
        effects.0.clear();
        assert_eq!(
            flow.continue_if_ready(
                ContinuationConditions {
                    function,
                    ..ContinuationConditions::default()
                },
                &mut effects
            ),
            ContinuationResult::Emitted(ScanEffect::AdvanceMemory)
        );
    }
}

#[test]
fn emitted_effect_consumes_the_schedule_once() {
    let (mut flow, mut effects) = scheduled_flow(ScanKind::Frequency);
    effects.0.clear();
    let conditions = ContinuationConditions::default();
    assert_eq!(
        flow.continue_if_ready(conditions, &mut effects),
        ContinuationResult::Emitted(ScanEffect::AdvanceFrequency)
    );
    assert!(!flow.state().is_scheduled());
    assert_eq!(
        flow.continue_if_ready(conditions, &mut effects),
        ContinuationResult::NotScheduled
    );
    assert_eq!(effects.0, [ScanEffect::AdvanceFrequency]);
}

#[test]
fn disabled_voice_gate_ignores_a_nonempty_queue() {
    let (mut flow, mut effects) = scheduled_flow(ScanKind::Frequency);
    effects.0.clear();
    assert_eq!(
        flow.continue_if_ready(
            ContinuationConditions {
                voice_enabled: false,
                voice_queue_empty: false,
                ..ContinuationConditions::default()
            },
            &mut effects
        ),
        ContinuationResult::Emitted(ScanEffect::AdvanceFrequency)
    );
}

#[test]
fn state_events_remain_owned_by_the_existing_scan_model() {
    let mut flow = AppScanFlow::new();
    let mut effects = Effects::default();
    flow.start(ScanKind::Memory, ResumeMode::Time, true, &mut effects);

    flow.detected();
    assert_eq!(flow.state().reception_state(), ReceptionState::Detected);
    assert!(flow.state().is_detection_latched());
    flow.found();
    assert_eq!(flow.state().reception_state(), ReceptionState::Listening);
    assert!(flow.state().is_paused());
    assert!(flow.state().keeps_last_result());
    flow.reception_ended();
    assert_eq!(flow.state().reception_state(), ReceptionState::None);
    assert_eq!(
        flow.stop(),
        ScanAction::Stopped(StopDisposition::KeepLastFound)
    );
    assert_eq!(flow.kind(), None);

    flow.start(ScanKind::Frequency, ResumeMode::Carrier, true, &mut effects);
    flow.found();
    flow.reception_ended();
    assert_eq!(
        flow.state().remaining_10ms(),
        ScanState::CARRIER_RESUME_DELAY_10MS
    );
    flow.tick(FunctionState::Transmit);
    assert_eq!(
        flow.state().remaining_10ms(),
        ScanState::CARRIER_RESUME_DELAY_10MS
    );
    flow.stop();

    flow.start(ScanKind::Frequency, ResumeMode::Search, true, &mut effects);
    flow.found();
    assert_eq!(
        flow.reception_ended(),
        ScanAction::Stopped(StopDisposition::KeepLastFound)
    );
    assert_eq!(flow.kind(), None);
}
