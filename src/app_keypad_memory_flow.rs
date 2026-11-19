#![cfg_attr(not(test), no_std)]

//! Offline composition of keypad polling and memory-channel numeric entry.

#[allow(dead_code)]
#[allow(unused_attributes)]
#[path = "app_numeric_input_flow.rs"]
mod app_numeric_input_flow;
#[allow(unused_attributes)]
#[path = "keyboard_matrix_scan.rs"]
mod keyboard_matrix_scan;

use app_numeric_input_flow::KeyboardDecisions;
use app_numeric_input_flow::process_memory_channel_key_event;
pub use app_numeric_input_flow::{Digit, FlowEffect, Key, KeyEvent, NumericInputBox};
use keyboard_matrix_scan::Key as MatrixKey;
pub use keyboard_matrix_scan::MatrixIo;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryChannelContext {
    pub memory_mode: bool,
    pub f_key_pressed: bool,
    pub is_active_tx_vfo: bool,
    pub selected_vfo: u8,
}

impl MemoryChannelContext {
    const fn is_eligible(self) -> bool {
        self.memory_mode && self.is_active_tx_vfo
    }
}

pub trait KeypadMemoryEffects {
    fn cancel_boot_counter(&mut self);
    fn memory_effect(&mut self, effect: FlowEffect);
}

pub struct KeypadMemoryFlow {
    decisions: KeyboardDecisions,
    input_box: NumericInputBox,
}

impl KeypadMemoryFlow {
    pub const fn new() -> Self {
        Self {
            decisions: KeyboardDecisions::new(),
            input_box: NumericInputBox::new(),
        }
    }

    pub fn input_box(&self) -> &NumericInputBox {
        &self.input_box
    }

    /// Poll once per 10 ms; the caller owns cadence and application state.
    pub fn poll(
        &mut self,
        io: &mut impl MatrixIo,
        context: MemoryChannelContext,
        effects: &mut impl KeypadMemoryEffects,
        is_valid_channel: impl FnMut(u16) -> bool,
    ) -> Option<KeyEvent> {
        let matrix_key = keyboard_matrix_scan::poll(io);
        if matrix_key.is_some() {
            effects.cancel_boot_counter();
        }

        let event = self.decisions.sample(matrix_key.map(logical_key));
        if let Some(event) = event
            && context.is_eligible()
        {
            let memory_effects = process_memory_channel_key_event(
                &mut self.input_box,
                event,
                context.f_key_pressed,
                context.selected_vfo,
                is_valid_channel,
            );
            for effect in memory_effects.iter() {
                effects.memory_effect(effect);
            }
        }
        event
    }
}

impl Default for KeypadMemoryFlow {
    fn default() -> Self {
        Self::new()
    }
}

fn logical_key(key: MatrixKey) -> Key {
    match key {
        MatrixKey::Digit0 => Key::Digit0,
        MatrixKey::Digit1 => Key::Digit1,
        MatrixKey::Digit2 => Key::Digit2,
        MatrixKey::Digit3 => Key::Digit3,
        MatrixKey::Digit4 => Key::Digit4,
        MatrixKey::Digit5 => Key::Digit5,
        MatrixKey::Digit6 => Key::Digit6,
        MatrixKey::Digit7 => Key::Digit7,
        MatrixKey::Digit8 => Key::Digit8,
        MatrixKey::Digit9 => Key::Digit9,
        MatrixKey::Menu => Key::Menu,
        MatrixKey::Up => Key::Up,
        MatrixKey::Down => Key::Down,
        MatrixKey::Exit => Key::Exit,
        MatrixKey::Star => Key::Star,
        MatrixKey::F => Key::F,
        MatrixKey::Side2 => Key::Side2,
        MatrixKey::Side1 => Key::Side1,
    }
}
