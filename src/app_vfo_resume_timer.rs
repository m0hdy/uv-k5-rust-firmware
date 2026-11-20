//! VFO resume countdown transition from the pinned application 500 ms slice.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct State {
    pub resume_countdown: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Context {
    pub ptt_pressed: bool,
    pub fm_feature_enabled: bool,
    pub fm_radio_mode: bool,
    pub receiving: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Effects {
    pub restore_normal_vfo_state: bool,
    pub restart_fm: bool,
    pub select_fm_display: bool,
}

/// Decrements the resume timer unless PTT is pressed and reports expiry actions.
pub fn advance(state: &mut State, context: Context) -> Effects {
    if context.ptt_pressed || state.resume_countdown == 0 {
        return Effects::default();
    }

    state.resume_countdown -= 1;
    if state.resume_countdown != 0 {
        return Effects::default();
    }

    let resume_fm = context.fm_feature_enabled && context.fm_radio_mode && !context.receiving;
    Effects {
        restore_normal_vfo_state: true,
        restart_fm: resume_fm,
        select_fm_display: resume_fm,
    }
}
