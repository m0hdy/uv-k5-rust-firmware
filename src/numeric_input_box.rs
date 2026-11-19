#![cfg_attr(not(test), no_std)]

pub const SLOT_COUNT: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Digit {
    Zero,
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
}

impl Digit {
    const fn ascii(self) -> u8 {
        b'0' + self as u8
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Slot {
    Digit(Digit),
    Placeholder,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputEvent {
    Digit(Digit),
    NoDigit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NumericInputBox {
    slots: [Slot; SLOT_COUNT],
    index: usize,
}

impl NumericInputBox {
    pub const fn new() -> Self {
        Self {
            slots: [Slot::Digit(Digit::Zero); SLOT_COUNT],
            index: 0,
        }
    }

    pub const fn index(&self) -> usize {
        self.index
    }

    pub const fn slots(&self) -> &[Slot; SLOT_COUNT] {
        &self.slots
    }

    pub fn append(&mut self, event: InputEvent) {
        if self.index >= SLOT_COUNT {
            return;
        }

        if self.index == 0 {
            self.slots = [Slot::Placeholder; SLOT_COUNT];
        }

        if let InputEvent::Digit(digit) = event {
            self.slots[self.index] = Slot::Digit(digit);
            self.index += 1;
        }
    }

    pub fn reset_index(&mut self) {
        self.index = 0;
    }

    pub fn ascii_projection(&self) -> [u8; SLOT_COUNT] {
        let mut ascii = [0; SLOT_COUNT];
        let mut index = 0;
        while index < SLOT_COUNT {
            ascii[index] = match self.slots[index] {
                Slot::Digit(digit) => digit.ascii(),
                Slot::Placeholder => b'-',
            };
            index += 1;
        }
        ascii
    }
}

impl Default for NumericInputBox {
    fn default() -> Self {
        Self::new()
    }
}
