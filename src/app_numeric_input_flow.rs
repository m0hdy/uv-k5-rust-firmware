#![cfg_attr(not(test), no_std)]

#[allow(dead_code)]
#[path = "keyboard_decisions.rs"]
mod keyboard_decisions;
#[allow(dead_code, unused_attributes)]
#[path = "numeric_input_box.rs"]
mod numeric_input_box;

#[allow(unused_imports)]
pub use keyboard_decisions::KeyboardDecisions;
pub use keyboard_decisions::{Key, KeyEvent};
pub use numeric_input_box::{Digit, NumericInputBox};

const MAX_EFFECTS: usize = 9;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlowEffect {
    RequestOptionalKeyBeep,
    RefreshKeyInputTimeout,
    AppendDigit(Digit),
    RequestMainDisplay,
    ResetInputIndex,
    CheckChannelValidity(u16),
    RequestOptionalInvalidChannelBeep,
    UpdateSelectedVfoMemoryChannel { vfo: u8, channel: u8 },
    UpdateSelectedVfoScreenChannel { vfo: u8, channel: u8 },
    RequestSaveVfo,
    ConfigureVfoReload,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowEffects {
    effects: [Option<FlowEffect>; MAX_EFFECTS],
    len: usize,
}

impl FlowEffects {
    const fn new() -> Self {
        Self {
            effects: [None; MAX_EFFECTS],
            len: 0,
        }
    }

    fn push(&mut self, effect: FlowEffect) {
        if self.len < MAX_EFFECTS {
            self.effects[self.len] = Some(effect);
            self.len += 1;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = FlowEffect> + '_ {
        self.effects[..self.len].iter().flatten().copied()
    }
}

fn digit_for_key(key: Key) -> Option<Digit> {
    match key {
        Key::Digit0 => Some(Digit::Zero),
        Key::Digit1 => Some(Digit::One),
        Key::Digit2 => Some(Digit::Two),
        Key::Digit3 => Some(Digit::Three),
        Key::Digit4 => Some(Digit::Four),
        Key::Digit5 => Some(Digit::Five),
        Key::Digit6 => Some(Digit::Six),
        Key::Digit7 => Some(Digit::Seven),
        Key::Digit8 => Some(Digit::Eight),
        Key::Digit9 => Some(Digit::Nine),
        _ => None,
    }
}

fn entered_channel(input_box: &NumericInputBox) -> u16 {
    let slots = input_box.slots();
    let hundreds = match slots[0] {
        numeric_input_box::Slot::Digit(digit) => digit as u16,
        numeric_input_box::Slot::Placeholder => 0,
    };
    let tens = match slots[1] {
        numeric_input_box::Slot::Digit(digit) => digit as u16,
        numeric_input_box::Slot::Placeholder => 0,
    };
    let ones = match slots[2] {
        numeric_input_box::Slot::Digit(digit) => digit as u16,
        numeric_input_box::Slot::Placeholder => 0,
    };

    (hundreds * 100 + tens * 10 + ones).wrapping_sub(1)
}

/// Models the memory-channel branch of pinned `app/main.c` without owning
/// mode eligibility or firmware state.
pub fn process_memory_channel_key_event(
    input_box: &mut NumericInputBox,
    event: KeyEvent,
    f_key_pressed: bool,
    selected_vfo: u8,
    mut is_valid_channel: impl FnMut(u16) -> bool,
) -> FlowEffects {
    let mut effects = FlowEffects::new();

    if let KeyEvent::Pressed(key) = event {
        if digit_for_key(key).is_some() {
            effects.push(FlowEffect::RequestOptionalKeyBeep);
        }
        return effects;
    }

    let KeyEvent::Released {
        key,
        was_held: false,
    } = event
    else {
        return effects;
    };

    if f_key_pressed {
        return effects;
    }

    let Some(digit) = digit_for_key(key) else {
        return effects;
    };

    input_box.append(numeric_input_box::InputEvent::Digit(digit));
    effects.push(FlowEffect::RefreshKeyInputTimeout);
    effects.push(FlowEffect::AppendDigit(digit));
    effects.push(FlowEffect::RequestMainDisplay);

    if input_box.index() != 3 {
        return effects;
    }

    input_box.reset_index();
    effects.push(FlowEffect::ResetInputIndex);
    let channel = entered_channel(input_box);
    effects.push(FlowEffect::CheckChannelValidity(channel));

    if !is_valid_channel(channel) {
        effects.push(FlowEffect::RequestOptionalInvalidChannelBeep);
        return effects;
    }

    let Ok(channel) = u8::try_from(channel) else {
        effects.push(FlowEffect::RequestOptionalInvalidChannelBeep);
        return effects;
    };

    effects.push(FlowEffect::UpdateSelectedVfoMemoryChannel {
        vfo: selected_vfo,
        channel,
    });
    effects.push(FlowEffect::UpdateSelectedVfoScreenChannel {
        vfo: selected_vfo,
        channel,
    });
    effects.push(FlowEffect::RequestSaveVfo);
    effects.push(FlowEffect::ConfigureVfoReload);
    effects
}
