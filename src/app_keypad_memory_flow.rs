#![cfg_attr(not(test), no_std)]

//! Offline composition of keypad polling and memory-channel numeric entry.

#[allow(dead_code)]
#[allow(unused_attributes)]
#[path = "app_numeric_input_flow.rs"]
mod app_numeric_input_flow;

use app_numeric_input_flow::process_memory_channel_key_event;
pub use app_numeric_input_flow::{
    Digit, FlowEffect, Key, KeyEvent, KeyboardPollFlow, MatrixIo, NumericInputBox,
};

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

struct MemoryFlowAdapter<'a, E, V> {
    context: MemoryChannelContext,
    input_box: &'a mut NumericInputBox,
    effects: &'a mut E,
    is_valid_channel: V,
    event: Option<KeyEvent>,
}

impl<E, V> app_numeric_input_flow::KeypadEffects for MemoryFlowAdapter<'_, E, V>
where
    E: KeypadMemoryEffects,
    V: FnMut(u16) -> bool,
{
    fn cancel_boot_counter(&mut self) {
        self.effects.cancel_boot_counter();
    }

    fn key_event(&mut self, event: KeyEvent) {
        self.event = Some(event);
        if self.context.is_eligible() {
            let memory_effects = process_memory_channel_key_event(
                self.input_box,
                event,
                self.context.f_key_pressed,
                self.context.selected_vfo,
                &mut self.is_valid_channel,
            );
            for effect in memory_effects.iter() {
                self.effects.memory_effect(effect);
            }
        }
    }
}

pub struct KeypadMemoryFlow {
    keyboard: KeyboardPollFlow,
    input_box: NumericInputBox,
}

impl KeypadMemoryFlow {
    pub const fn new() -> Self {
        Self {
            keyboard: KeyboardPollFlow::new(),
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
        let mut adapter = MemoryFlowAdapter {
            context,
            input_box: &mut self.input_box,
            effects,
            is_valid_channel,
            event: None,
        };
        self.keyboard.poll(io, &mut adapter);
        adapter.event
    }
}

impl Default for KeypadMemoryFlow {
    fn default() -> Self {
        Self::new()
    }
}
