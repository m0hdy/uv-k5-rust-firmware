#![cfg_attr(not(test), no_std)]

#[allow(unused_attributes)]
#[path = "scan_decisions.rs"]
mod scan_decisions;

pub use scan_decisions::{
    FunctionState, ReceptionState, ResumeMode, ScanAction, ScanKind, ScanState, StopDisposition,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListenMode {
    Monitor,
    Receive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanEffect {
    AdvanceFrequency,
    AdvanceMemory,
    Listen(ListenMode),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContinuationResult {
    NotScheduled,
    Blocked,
    Emitted(ScanEffect),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContinuationConditions {
    pub function: FunctionState,
    pub code_type_off: bool,
    pub monitor: bool,
    pub reduced_service: bool,
    pub fm_radio_mode: bool,
    pub fm_countdown_pending: bool,
    pub background_scanner_active: bool,
    pub ptt_pressed: bool,
    pub voice_enabled: bool,
    pub voice_queue_empty: bool,
}

impl Default for ContinuationConditions {
    fn default() -> Self {
        Self {
            function: FunctionState::Other,
            code_type_off: false,
            monitor: false,
            reduced_service: false,
            fm_radio_mode: false,
            fm_countdown_pending: false,
            background_scanner_active: false,
            ptt_pressed: false,
            voice_enabled: false,
            voice_queue_empty: true,
        }
    }
}

pub trait ScanEffects {
    fn emit(&mut self, effect: ScanEffect);
}

pub struct AppScanFlow {
    state: ScanState,
    kind: Option<ScanKind>,
}

impl AppScanFlow {
    pub const fn new() -> Self {
        Self {
            state: ScanState::new(),
            kind: None,
        }
    }

    pub const fn state(&self) -> &ScanState {
        &self.state
    }

    pub const fn kind(&self) -> Option<ScanKind> {
        self.kind
    }

    pub fn start(
        &mut self,
        kind: ScanKind,
        resume_mode: ResumeMode,
        store_backup_settings: bool,
        effects: &mut impl ScanEffects,
    ) {
        self.kind = Some(kind);
        let _ = self.state.apply(scan_decisions::ScanEvent::Start {
            kind,
            resume_mode,
            store_backup_settings,
        });
        effects.emit(Self::advance_effect(kind));
    }

    pub fn tick(&mut self, function: FunctionState) -> ScanAction {
        self.state
            .apply(scan_decisions::ScanEvent::Tick { function })
    }

    pub fn detected(&mut self) -> ScanAction {
        self.state.apply(scan_decisions::ScanEvent::Detected)
    }

    pub fn found(&mut self) -> ScanAction {
        self.state.apply(scan_decisions::ScanEvent::Found)
    }

    pub fn reception_ended(&mut self) -> ScanAction {
        let action = self.state.apply(scan_decisions::ScanEvent::ReceptionEnded);
        if matches!(action, ScanAction::Stopped(_)) {
            self.kind = None;
        }
        action
    }

    pub fn stop(&mut self) -> ScanAction {
        let action = self.state.apply(scan_decisions::ScanEvent::Stop);
        if matches!(action, ScanAction::Stopped(_)) {
            self.kind = None;
        }
        action
    }

    pub fn continue_if_ready(
        &mut self,
        conditions: ContinuationConditions,
        effects: &mut impl ScanEffects,
    ) -> ContinuationResult {
        if !self.state.is_active() || !self.state.is_scheduled() {
            return ContinuationResult::NotScheduled;
        }

        if conditions.reduced_service
            || (conditions.fm_radio_mode && conditions.fm_countdown_pending)
            || conditions.background_scanner_active
            || conditions.ptt_pressed
            || (conditions.voice_enabled && !conditions.voice_queue_empty)
        {
            return ContinuationResult::Blocked;
        }

        let Some(kind) = self.kind else {
            return ContinuationResult::NotScheduled;
        };
        let mut next_state = self.state;
        let action = next_state.apply(scan_decisions::ScanEvent::Continue {
            function: conditions.function,
            code_type_off: conditions.code_type_off,
        });
        let effect = match action {
            ScanAction::Advance => Self::advance_effect(kind),
            ScanAction::Listen => ScanEffect::Listen(if conditions.monitor {
                ListenMode::Monitor
            } else {
                ListenMode::Receive
            }),
            ScanAction::None | ScanAction::Stopped(_) => {
                return ContinuationResult::NotScheduled;
            }
        };

        effects.emit(effect);
        self.state = next_state;
        ContinuationResult::Emitted(effect)
    }

    const fn advance_effect(kind: ScanKind) -> ScanEffect {
        match kind {
            ScanKind::Frequency => ScanEffect::AdvanceFrequency,
            ScanKind::Memory => ScanEffect::AdvanceMemory,
        }
    }
}

impl Default for AppScanFlow {
    fn default() -> Self {
        Self::new()
    }
}
