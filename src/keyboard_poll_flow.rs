//! Offline composition of matrix polling and 10 ms application key decisions.

#[path = "keyboard_decisions.rs"]
mod keyboard_decisions;
#[allow(unused_attributes)]
#[path = "keyboard_matrix_scan.rs"]
mod keyboard_matrix_scan;

use keyboard_decisions::KeyboardDecisions;
pub use keyboard_decisions::{Key, KeyEvent};
use keyboard_matrix_scan::Key as MatrixKey;
pub use keyboard_matrix_scan::MatrixIo;

/// Application effects, delivered in the order they occur during a poll.
pub trait KeypadEffects {
    fn cancel_boot_counter(&mut self);
    fn key_event(&mut self, event: KeyEvent);
}

pub struct KeyboardPollFlow {
    decisions: KeyboardDecisions,
}

impl KeyboardPollFlow {
    pub const fn new() -> Self {
        Self {
            decisions: KeyboardDecisions::new(),
        }
    }

    /// Poll once per 10 ms; the caller owns the cadence.
    pub fn poll(&mut self, io: &mut impl MatrixIo, effects: &mut impl KeypadEffects) {
        let key = keyboard_matrix_scan::poll(io).map(decision_key);
        if key.is_some() {
            effects.cancel_boot_counter();
        }
        if let Some(event) = self.decisions.sample(key) {
            effects.key_event(event);
        }
    }
}

impl Default for KeyboardPollFlow {
    fn default() -> Self {
        Self::new()
    }
}

fn decision_key(key: MatrixKey) -> Key {
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
