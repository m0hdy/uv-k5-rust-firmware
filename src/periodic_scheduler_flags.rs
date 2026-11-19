#![no_std]

pub struct PeriodicSchedulerFlags {
    counter: u32,
    pending_10ms: bool,
    pending_40ms: bool,
    pending_500ms: bool,
}

impl PeriodicSchedulerFlags {
    pub const fn new() -> Self {
        Self::with_counter(0)
    }

    pub const fn with_counter(counter: u32) -> Self {
        Self {
            counter,
            pending_10ms: false,
            pending_40ms: false,
            pending_500ms: false,
        }
    }

    pub const fn counter(&self) -> u32 {
        self.counter
    }

    pub const fn pending_10ms(&self) -> bool {
        self.pending_10ms
    }

    pub const fn pending_40ms(&self) -> bool {
        self.pending_40ms
    }

    pub const fn pending_500ms(&self) -> bool {
        self.pending_500ms
    }

    pub fn tick(&mut self) {
        self.counter = self.counter.wrapping_add(1);
        self.pending_10ms = true;

        if self.counter.is_multiple_of(50) {
            self.pending_500ms = true;
        }

        if self.counter & 3 == 0 {
            self.pending_40ms = true;
        }
    }

    pub fn acknowledge_10ms(&mut self) {
        self.pending_10ms = false;
    }

    pub fn acknowledge_40ms(&mut self) {
        self.pending_40ms = false;
    }

    pub fn acknowledge_500ms(&mut self) {
        self.pending_500ms = false;
    }
}
