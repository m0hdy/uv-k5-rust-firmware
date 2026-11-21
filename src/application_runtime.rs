//! Application-owned input and 500 ms state transitions.

use crate::app_500ms_service_timers::{
    self, BacklightMenuItem, Context as ServiceContext, State as ServiceState,
};
use crate::app_500ms_timeouts::{
    self, CenterLine, Context as TimeoutContext, State as TimeoutState,
};
use crate::keyboard_poll_flow::{Key, KeyEvent};
use crate::numeric_input_box::{Digit, InputEvent, NumericInputBox};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Context {
    pub enable_rssi_bar: bool,
    pub center_line: CenterLine,
    pub enable_fmradio: bool,
    pub fm_radio_mode: bool,
    pub ask_to_save: bool,
    pub css_background_scan: bool,
    pub backlight_menu_item: BacklightMenuItem,
    pub backlight_time: u8,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Effects {
    pub enter_menu: bool,
    pub exit_menu: bool,
    pub return_from_timeslice: bool,
    pub turn_backlight_off: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct State {
    input_box: NumericInputBox,
    timeouts: TimeoutState,
    service_timers: ServiceState,
    screen_is_menu: bool,
    last_key_event: Option<KeyEvent>,
    key_event_count: u16,
}

impl State {
    pub const fn new() -> Self {
        Self {
            input_box: NumericInputBox::new(),
            timeouts: TimeoutState {
                keypad_locked_countdown: 0,
                dtmf_live_timeout: 0,
                dtmf_live_text: [0; 20],
                menu_countdown: 0,
                update_display: false,
            },
            service_timers: ServiceState {
                fm_radio_countdown: 0,
                backlight_countdown: 0,
            },
            screen_is_menu: false,
            last_key_event: None,
            key_event_count: 0,
        }
    }

    pub const fn input_box(&self) -> &NumericInputBox {
        &self.input_box
    }

    pub const fn screen_is_menu(&self) -> bool {
        self.screen_is_menu
    }

    pub const fn last_key_event(&self) -> Option<KeyEvent> {
        self.last_key_event
    }

    pub const fn key_event_count(&self) -> u16 {
        self.key_event_count
    }

    pub const fn update_display(&self) -> bool {
        self.timeouts.update_display
    }

    pub fn set_countdowns(
        &mut self,
        keypad_locked: u8,
        dtmf_live: u8,
        menu: u16,
        fm_radio: u8,
        backlight: u16,
    ) {
        self.timeouts.keypad_locked_countdown = keypad_locked;
        self.timeouts.dtmf_live_timeout = dtmf_live;
        self.timeouts.menu_countdown = menu;
        self.service_timers.fm_radio_countdown = fm_radio;
        self.service_timers.backlight_countdown = backlight;
    }

    pub fn on_key_event(&mut self, event: KeyEvent, menu_timeout: u16) -> Effects {
        self.last_key_event = Some(event);
        self.key_event_count = self.key_event_count.wrapping_add(1);
        self.timeouts.update_display = true;

        let mut effects = Effects::default();
        match event {
            KeyEvent::Pressed(Key::Menu) if !self.screen_is_menu => {
                self.screen_is_menu = true;
                self.timeouts.menu_countdown = menu_timeout;
                effects.enter_menu = true;
            }
            KeyEvent::Pressed(Key::Exit) if self.screen_is_menu => {
                self.screen_is_menu = false;
                self.timeouts.menu_countdown = 0;
                self.input_box.reset_index();
                effects.exit_menu = true;
            }
            KeyEvent::Released {
                key,
                was_held: false,
            } => {
                if let Some(digit) = digit_for_key(key) {
                    self.input_box.append(InputEvent::Digit(digit));
                }
            }
            KeyEvent::Pressed(_)
            | KeyEvent::Released { was_held: true, .. }
            | KeyEvent::Held(_) => {}
        }
        effects
    }

    pub fn timeslice_500ms(&mut self, context: Context) -> Effects {
        let timeout_effects = app_500ms_timeouts::advance(
            &mut self.timeouts,
            TimeoutContext {
                enable_rssi_bar: context.enable_rssi_bar,
                center_line: context.center_line,
                screen_is_menu: self.screen_is_menu,
            },
        );
        let service_effects = app_500ms_service_timers::advance(
            &mut self.service_timers,
            ServiceContext {
                enable_fmradio: context.enable_fmradio,
                fm_radio_mode: context.fm_radio_mode,
                ask_to_save: context.ask_to_save,
                css_background_scan: context.css_background_scan,
                screen_is_menu: self.screen_is_menu,
                menu_item: context.backlight_menu_item,
                backlight_time: context.backlight_time,
            },
        );
        if timeout_effects.exit_menu {
            self.screen_is_menu = false;
            self.timeouts.menu_countdown = 0;
        }
        Effects {
            enter_menu: false,
            exit_menu: timeout_effects.exit_menu,
            return_from_timeslice: service_effects.return_from_timeslice,
            turn_backlight_off: service_effects.turn_backlight_off,
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

fn digit_for_key(key: Key) -> Option<Digit> {
    Some(match key {
        Key::Digit0 => Digit::Zero,
        Key::Digit1 => Digit::One,
        Key::Digit2 => Digit::Two,
        Key::Digit3 => Digit::Three,
        Key::Digit4 => Digit::Four,
        Key::Digit5 => Digit::Five,
        Key::Digit6 => Digit::Six,
        Key::Digit7 => Digit::Seven,
        Key::Digit8 => Digit::Eight,
        Key::Digit9 => Digit::Nine,
        _ => return None,
    })
}
