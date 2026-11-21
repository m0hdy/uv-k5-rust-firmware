//! Pure caller-owned timeout mutations from the pinned application 500 ms slice.
//!
//! This module does not schedule the slice, render UI, play audio, or access
//! hardware. The caller supplies feature and current-UI context and applies the
//! returned menu-exit decision.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CenterLine {
    None = 0,
    InUse = 1,
    AudioBar = 2,
    Rssi = 3,
    AmFixData = 4,
    DtmfDecode = 5,
    ChargeData = 6,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Context {
    pub enable_rssi_bar: bool,
    pub center_line: CenterLine,
    pub screen_is_menu: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct State {
    pub keypad_locked_countdown: u8,
    pub dtmf_live_timeout: u8,
    pub dtmf_live_text: [u8; 20],
    pub menu_countdown: u16,
    pub update_display: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Effects {
    pub exit_menu: bool,
}

/// Applies one pinned `APP_TimeSlice500ms` timeout prefix in source order.
///
/// `update_display` is sticky: this routine only sets it and never clears it.
pub fn advance(state: &mut State, context: Context) -> Effects {
    if state.keypad_locked_countdown > 0 {
        state.keypad_locked_countdown -= 1;
        if state.keypad_locked_countdown == 0 {
            state.update_display = true;
        }
    }

    let dtmf_line_available = !context.enable_rssi_bar
        || matches!(
            context.center_line,
            CenterLine::None | CenterLine::DtmfDecode
        );
    if state.dtmf_live_timeout > 0 && dtmf_line_available {
        state.dtmf_live_timeout -= 1;
        if state.dtmf_live_timeout == 0 && state.dtmf_live_text[0] != 0 {
            state.dtmf_live_text.fill(0);
            state.update_display = true;
        }
    }

    let mut effects = Effects::default();
    if state.menu_countdown > 0 {
        state.menu_countdown -= 1;
        if state.menu_countdown == 0 {
            effects.exit_menu = context.screen_is_menu;
        }
    }

    effects
}
