#![cfg_attr(not(test), no_std)]

#[allow(unused_attributes)]
#[path = "app_timeslice_dispatch.rs"]
mod app_timeslice_dispatch;
#[allow(unused_attributes)]
#[path = "periodic_scheduler_flags.rs"]
mod periodic_scheduler_flags;

use app_timeslice_dispatch::Backend;
use periodic_scheduler_flags::PeriodicSchedulerFlags;

/// Application effects used by one main-loop iteration.
pub trait ApplicationEffects {
    fn update(&mut self);
    fn timeslice_10ms(&mut self, deliver_tick: &mut dyn FnMut());
    fn timeslice_500ms(&mut self);
}

/// Composes abstract tick delivery with one application-loop iteration.
pub struct AppSchedulerComposition {
    flags: PeriodicSchedulerFlags,
}

impl AppSchedulerComposition {
    pub const fn new() -> Self {
        Self {
            flags: PeriodicSchedulerFlags::new(),
        }
    }

    pub fn tick(&mut self) {
        self.flags.tick();
    }

    pub fn acknowledge_40ms(&mut self) {
        self.flags.acknowledge_40ms();
    }

    pub const fn counter(&self) -> u32 {
        self.flags.counter()
    }

    pub const fn pending_10ms(&self) -> bool {
        self.flags.pending_10ms()
    }

    pub const fn pending_40ms(&self) -> bool {
        self.flags.pending_40ms()
    }

    pub const fn pending_500ms(&self) -> bool {
        self.flags.pending_500ms()
    }

    pub fn run_iteration(&mut self, effects: &mut impl ApplicationEffects) {
        let mut backend = CompositionBackend {
            flags: &mut self.flags,
            effects,
        };
        app_timeslice_dispatch::run(&mut backend);
    }
}

struct CompositionBackend<'a, E> {
    flags: &'a mut PeriodicSchedulerFlags,
    effects: &'a mut E,
}

impl<E: ApplicationEffects> Backend for CompositionBackend<'_, E> {
    fn update(&mut self) {
        self.effects.update();
    }

    fn is_10ms_pending(&mut self) -> bool {
        self.flags.pending_10ms()
    }

    fn timeslice_10ms(&mut self) {
        self.flags.acknowledge_10ms();
        let flags = &mut self.flags;
        self.effects.timeslice_10ms(&mut || flags.tick());
    }

    fn is_500ms_pending(&mut self) -> bool {
        self.flags.pending_500ms()
    }

    fn timeslice_500ms(&mut self) {
        self.flags.acknowledge_500ms();
        self.effects.timeslice_500ms();
    }
}
