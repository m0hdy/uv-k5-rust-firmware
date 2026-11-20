//! Pure FM and backlight timer decisions from the pinned application 500 ms slice.

/// The three menu items that affect the pinned backlight timeout gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BacklightMenuItem {
    Other,
    AutoBrightness,
    MinimumBrightness,
    MaximumBrightness,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Context {
    pub enable_fmradio: bool,
    pub fm_radio_mode: bool,
    pub ask_to_save: bool,
    pub css_background_scan: bool,
    pub screen_is_menu: bool,
    pub menu_item: BacklightMenuItem,
    pub backlight_time: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct State {
    pub fm_radio_countdown: u8,
    pub backlight_countdown: u16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Effects {
    pub return_from_timeslice: bool,
    pub turn_backlight_off: bool,
}

/// `gSubMenu_BACKLIGHT` has eight options and its final option is `ON`.
pub const BACKLIGHT_ALWAYS_ON_INDEX: u8 = 7;

/// Advances the FM and backlight timers in their pinned source order.
pub fn advance(state: &mut State, context: Context) -> Effects {
    if context.enable_fmradio && state.fm_radio_countdown > 0 {
        state.fm_radio_countdown -= 1;
        if context.fm_radio_mode {
            return Effects {
                return_from_timeslice: true,
                turn_backlight_off: false,
            };
        }
    }

    let backlight_menu_item = context.screen_is_menu
        && matches!(
            context.menu_item,
            BacklightMenuItem::AutoBrightness | BacklightMenuItem::MaximumBrightness
        );
    if state.backlight_countdown > 0
        && !context.ask_to_save
        && !context.css_background_scan
        && !backlight_menu_item
    {
        state.backlight_countdown -= 1;
        if state.backlight_countdown == 0 && context.backlight_time < BACKLIGHT_ALWAYS_ON_INDEX {
            return Effects {
                return_from_timeslice: false,
                turn_backlight_off: true,
            };
        }
    }

    Effects::default()
}
