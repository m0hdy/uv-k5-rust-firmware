//! Pure key decisions for one sampled key per 10 ms tick.
//!
//! The independently authored state machine follows the behavior in the
//! pinned C reference's [`app/app.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/app/app.c#L1020-L1085),
//! with timing values from [`misc.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/misc.c#L38-L43)
//! and the 10 ms scheduler cadence from
//! [`scheduler.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/scheduler.c#L38-L48).
//! The reference is pinned at commit
//! `7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`.
//!
//! The pinned reference's default
//! [`Makefile`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/Makefile)
//! build uses `-Os -Wall -Werror -Wextra -mcpu=cortex-m0`, `-fno-builtin`,
//! `-fshort-enums`, `-fno-delete-null-pointer-checks`, `-std=c2x`, and
//! `-flto=auto`; the key-decision block is not guarded by an `ENABLE_*` feature
//! flag. This implementation uses no C code, lookup table, GPIO, MMIO,
//! scheduler, or application-level action dispatch.

pub const SAMPLE_PERIOD_MS: u16 = 10;
pub const DEBOUNCE_TICKS: u16 = 20 / SAMPLE_PERIOD_MS;
pub const INITIAL_REPEAT_TICKS: u16 = 400 / SAMPLE_PERIOD_MS;
pub const REPEAT_INTERVAL_TICKS: u16 = 80 / SAMPLE_PERIOD_MS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub enum Key {
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Menu,
    Up,
    Down,
    Exit,
    Star,
    F,
    Ptt,
    Side2,
    Side1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyEvent {
    Pressed(Key),
    Released { key: Key, was_held: bool },
    Held(Key),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyboardDecisions {
    sampled_key: Option<Key>,
    debounced_key: Option<Key>,
    ticks: u16,
    held: bool,
}

impl KeyboardDecisions {
    pub const fn new() -> Self {
        Self {
            sampled_key: None,
            debounced_key: None,
            ticks: 0,
            held: false,
        }
    }

    pub fn sample(&mut self, sampled_key: Option<Key>) -> Option<KeyEvent> {
        if self.sampled_key != sampled_key {
            let release = if self.sampled_key.is_some() && sampled_key.is_some() {
                self.debounced_key.map(|key| KeyEvent::Released {
                    key,
                    was_held: self.held,
                })
            } else {
                None
            };
            self.sampled_key = sampled_key;
            self.ticks = 0;
            return release;
        }

        self.ticks = self.ticks.wrapping_add(1);

        if self.ticks == DEBOUNCE_TICKS {
            let event = match sampled_key {
                Some(key) => {
                    self.debounced_key = Some(key);
                    Some(KeyEvent::Pressed(key))
                }
                None => self.debounced_key.take().map(|key| KeyEvent::Released {
                    key,
                    was_held: self.held,
                }),
            };
            self.held = false;
            return event;
        }

        if self.ticks < INITIAL_REPEAT_TICKS || sampled_key.is_none() {
            return None;
        }

        let key = sampled_key.unwrap();
        if self.ticks == INITIAL_REPEAT_TICKS {
            if key == Key::Ptt {
                None
            } else {
                self.held = true;
                Some(KeyEvent::Held(key))
            }
        } else {
            let event = if matches!(key, Key::Up | Key::Down)
                && self.ticks.is_multiple_of(REPEAT_INTERVAL_TICKS)
            {
                self.held = true;
                Some(KeyEvent::Held(key))
            } else {
                None
            };
            if self.ticks == u16::MAX {
                self.ticks = INITIAL_REPEAT_TICKS + 1;
            }
            event
        }
    }
}

impl Default for KeyboardDecisions {
    fn default() -> Self {
        Self::new()
    }
}
