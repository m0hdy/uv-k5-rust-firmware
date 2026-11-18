//! Independently modeled scan decisions based on the pinned C scan, app, and
//! scheduler behavior at commit 7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResumeMode {
    Time,
    Carrier,
    Search,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanKind {
    Frequency,
    Memory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionState {
    Other,
    Incoming,
    Monitor,
    Transmit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceptionState {
    None,
    Detected,
    Listening,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanEvent {
    Start {
        kind: ScanKind,
        resume_mode: ResumeMode,
        store_backup_settings: bool,
    },
    Tick {
        function: FunctionState,
    },
    Detected,
    Found,
    ReceptionEnded,
    Continue {
        function: FunctionState,
        code_type_off: bool,
    },
    Stop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopDisposition {
    RestoreInitial,
    KeepLastFound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanAction {
    None,
    Advance,
    Listen,
    Stopped(StopDisposition),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanState {
    active: bool,
    kind: Option<ScanKind>,
    resume_mode: Option<ResumeMode>,
    pause_mode: bool,
    remaining_10ms: u16,
    schedule_listen: bool,
    keep_result: bool,
    reception_state: ReceptionState,
}

impl ScanState {
    pub const START_DELAY_10MS: u16 = 50;
    pub const NEXT_CHANNEL_DELAY_10MS: u16 = 9;
    pub const DETECTION_DELAY_10MS: u16 = 20;
    pub const TIME_RESUME_DELAY_10MS: u16 = 500;
    pub const CARRIER_RESUME_DELAY_10MS: u16 = 360;

    pub const fn new() -> Self {
        Self {
            active: false,
            kind: None,
            resume_mode: None,
            pause_mode: false,
            remaining_10ms: 0,
            schedule_listen: false,
            keep_result: false,
            reception_state: ReceptionState::None,
        }
    }

    pub const fn is_active(&self) -> bool {
        self.active
    }

    pub const fn is_paused(&self) -> bool {
        self.pause_mode
    }

    pub const fn is_scheduled(&self) -> bool {
        self.schedule_listen
    }

    pub const fn remaining_10ms(&self) -> u16 {
        self.remaining_10ms
    }

    pub const fn keeps_last_result(&self) -> bool {
        self.keep_result
    }

    pub const fn reception_state(&self) -> ReceptionState {
        self.reception_state
    }

    pub fn apply(&mut self, event: ScanEvent) -> ScanAction {
        match event {
            ScanEvent::Start {
                kind,
                resume_mode,
                store_backup_settings,
            } => {
                self.active = true;
                self.kind = Some(kind);
                self.resume_mode = Some(resume_mode);
                self.pause_mode = false;
                self.remaining_10ms = Self::START_DELAY_10MS;
                self.schedule_listen = false;
                self.reception_state = ReceptionState::None;
                if store_backup_settings {
                    self.keep_result = false;
                }
                ScanAction::None
            }
            ScanEvent::Tick { function } => {
                if self.active
                    && !matches!(function, FunctionState::Monitor | FunctionState::Transmit)
                    && self.remaining_10ms > 0
                {
                    self.remaining_10ms -= 1;
                    if self.remaining_10ms == 0 {
                        self.schedule_listen = true;
                    }
                }
                ScanAction::None
            }
            ScanEvent::Detected => {
                if self.active && self.reception_state == ReceptionState::None {
                    self.reception_state = ReceptionState::Detected;
                    self.remaining_10ms = Self::DETECTION_DELAY_10MS;
                    self.schedule_listen = false;
                }
                ScanAction::None
            }
            ScanEvent::Found => {
                if !self.active {
                    return ScanAction::None;
                }

                self.reception_state = ReceptionState::Listening;
                match self.resume_mode {
                    Some(ResumeMode::Time) if !self.pause_mode => {
                        self.remaining_10ms = Self::TIME_RESUME_DELAY_10MS;
                        self.schedule_listen = false;
                        self.pause_mode = true;
                    }
                    Some(ResumeMode::Carrier | ResumeMode::Search) => {
                        self.remaining_10ms = 0;
                        self.schedule_listen = false;
                    }
                    Some(ResumeMode::Time) | None => {}
                }
                self.keep_result = true;
                ScanAction::None
            }
            ScanEvent::ReceptionEnded => {
                if !self.active {
                    return ScanAction::None;
                }
                self.reception_state = ReceptionState::None;
                match self.resume_mode {
                    Some(ResumeMode::Carrier) => {
                        self.remaining_10ms = Self::CARRIER_RESUME_DELAY_10MS;
                        self.schedule_listen = false;
                        ScanAction::None
                    }
                    Some(ResumeMode::Search) => self.stop(),
                    Some(ResumeMode::Time) | None => ScanAction::None,
                }
            }
            ScanEvent::Continue {
                function,
                code_type_off,
            } => {
                if !self.active || !self.schedule_listen {
                    return ScanAction::None;
                }

                let listen = function == FunctionState::Incoming
                    && (self.kind == Some(ScanKind::Frequency) || code_type_off);
                if listen {
                    self.apply(ScanEvent::Found);
                }
                self.pause_mode = false;
                self.reception_state = ReceptionState::None;
                self.schedule_listen = false;
                if listen {
                    ScanAction::Listen
                } else {
                    self.remaining_10ms = Self::NEXT_CHANNEL_DELAY_10MS;
                    ScanAction::Advance
                }
            }
            ScanEvent::Stop => self.stop(),
        }
    }

    fn stop(&mut self) -> ScanAction {
        let disposition = if self.keep_result {
            StopDisposition::KeepLastFound
        } else {
            StopDisposition::RestoreInitial
        };
        self.active = false;
        self.kind = None;
        self.resume_mode = None;
        self.pause_mode = false;
        self.remaining_10ms = 0;
        self.schedule_listen = false;
        self.reception_state = ReceptionState::None;
        ScanAction::Stopped(disposition)
    }
}

impl Default for ScanState {
    fn default() -> Self {
        Self::new()
    }
}
