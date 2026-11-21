//! Offline up/down control transitions from the pinned main application.
//!
//! This module decides channel and frequency changes but leaves channel lookup,
//! tuning validation, scanning, persistence, audio, and radio I/O to its caller.

use crate::keyboard_poll_flow::{Key, KeyEvent};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Down,
    Up,
}

impl Direction {
    const fn signed(self) -> i8 {
        match self {
            Self::Down => -1,
            Self::Up => 1,
        }
    }
}

/// Mirrors `FREQUENCY_RoundToStep`, including the special 8.33 kHz channel
/// scheme and the legacy half-step used for step sizes of at least 1 kHz.
pub const fn round_frequency_to_step(frequency: u32, step: u16) -> Option<u32> {
    if step == 0 {
        return None;
    }
    if step == 833 {
        let base = frequency / 2_500 * 2_500;
        let channel = (frequency - base) / 700;
        return Some(base + channel * 833 + (channel == 3) as u32);
    }
    if step == 1 {
        return Some(frequency);
    }
    let step = if step >= 1_000 { step / 2 } else { step };
    let step = step as u32;
    let rounded = frequency.wrapping_add(step.div_ceil(2)) / step * step;
    Some(rounded)
}

/// Mirrors `APP_SetFreqByStepAndLimits` with explicit wrapping arithmetic.
pub fn set_frequency_by_step_and_limits(
    current: u32,
    direction: Direction,
    step: u16,
    lower: u32,
    upper: u32,
) -> Option<u32> {
    if step == 0 {
        return None;
    }
    let signed_step = i32::from(direction.signed()) * i32::from(step);
    let stepped = current.wrapping_add(signed_step as u32);
    let mut frequency = round_frequency_to_step(stepped, step)?;
    if frequency >= upper {
        frequency = lower;
    } else if frequency < lower {
        frequency = round_frequency_to_step(upper.wrapping_sub(u32::from(step)), step)?;
    }
    Some(frequency)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Context {
    pub screen_channel: u8,
    pub frequency_mode: bool,
    pub input_box_index: u8,
    pub scan_active: bool,
    pub noaa_enabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    OptionalKeyBeep,
    ShortKeyBeep,
    InvalidFrequencyBeep,
    SetFrequency(u32),
    TurnReceiverOn,
    RequestSaveChannel,
    SetMemoryChannel(u8),
    SetNoaaChannel(u8),
    RequestVoiceChannelNumber(u8),
    RequestSaveVfo,
    ConfigureVfoReload,
    StartScan(Direction),
    SetScanPauseCountdown(u16),
    ClearScheduledScanListen,
    MarkPttReleased,
}

const MAX_EFFECTS: usize = 9;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Effects {
    entries: [Option<Effect>; MAX_EFFECTS],
    len: usize,
}

impl Effects {
    const fn new() -> Self {
        Self {
            entries: [None; MAX_EFFECTS],
            len: 0,
        }
    }

    fn push(&mut self, effect: Effect) {
        if self.len < MAX_EFFECTS {
            self.entries[self.len] = Some(effect);
            self.len += 1;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = Effect> + '_ {
        self.entries[..self.len].iter().flatten().copied()
    }
}

/// Applies one pinned `MAIN_Key_UP_DOWN` event.
///
/// The caller supplies the exact step result and its receive-band validity,
/// plus the next valid memory channel. This keeps hardware-independent policy
/// testable while preserving the C helper calls at the adapter boundary.
pub fn process(
    context: Context,
    event: KeyEvent,
    mut step_frequency: impl FnMut(Direction) -> (u32, bool),
    mut next_memory_channel: impl FnMut(i16, Direction) -> Option<u8>,
) -> Effects {
    let mut effects = Effects::new();
    let (direction, held, pressed) = match event {
        KeyEvent::Pressed(Key::Up) => (Direction::Up, false, true),
        KeyEvent::Pressed(Key::Down) => (Direction::Down, false, true),
        KeyEvent::Held(Key::Up) => (Direction::Up, true, true),
        KeyEvent::Held(Key::Down) => (Direction::Down, true, true),
        KeyEvent::Released {
            key: Key::Up,
            was_held,
        } => (Direction::Up, was_held, false),
        KeyEvent::Released {
            key: Key::Down,
            was_held,
        } => (Direction::Down, was_held, false),
        _ => return effects,
    };

    if held || !pressed {
        if context.input_box_index > 0 {
            return effects;
        }
        if !pressed {
            if !held || context.frequency_mode {
                return effects;
            }
            effects.push(Effect::RequestVoiceChannelNumber(
                context.screen_channel.saturating_add(1),
            ));
            return effects;
        }
    } else {
        if context.input_box_index > 0 {
            effects.push(Effect::OptionalKeyBeep);
            return effects;
        }
        effects.push(Effect::ShortKeyBeep);
    }

    if context.scan_active {
        effects.push(Effect::StartScan(direction));
        effects.push(Effect::SetScanPauseCountdown(1));
        effects.push(Effect::ClearScheduledScanListen);
        effects.push(Effect::MarkPttReleased);
        return effects;
    }

    if context.noaa_enabled && is_noaa_channel(context.screen_channel) {
        let offset = context.screen_channel.saturating_sub(NOAA_CHANNEL_FIRST);
        let next = (i16::from(offset) + i16::from(direction.signed())).rem_euclid(10) as u8;
        let channel = NOAA_CHANNEL_FIRST + next;
        effects.push(Effect::SetNoaaChannel(channel));
    } else if context.frequency_mode {
        let (frequency, valid) = step_frequency(direction);
        if !valid {
            effects.push(Effect::InvalidFrequencyBeep);
            return effects;
        }
        effects.push(Effect::SetFrequency(frequency));
        effects.push(Effect::TurnReceiverOn);
        effects.push(Effect::RequestSaveChannel);
        return effects;
    } else if let Some(channel) = next_memory_channel(
        i16::from(context.screen_channel) + i16::from(direction.signed()),
        direction,
    ) {
        if channel == context.screen_channel {
            return effects;
        }
        effects.push(Effect::SetMemoryChannel(channel));
    } else {
        return effects;
    }

    effects.push(Effect::RequestSaveVfo);
    effects.push(Effect::ConfigureVfoReload);
    effects
}

const NOAA_CHANNEL_FIRST: u8 = 207;
const NOAA_CHANNEL_LAST: u8 = 216;

const fn is_noaa_channel(channel: u8) -> bool {
    channel >= NOAA_CHANNEL_FIRST && channel <= NOAA_CHANNEL_LAST
}
