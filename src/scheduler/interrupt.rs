//! Shared SysTick flags for the target main loop.
//!
//! The Cortex-M0 target has no Rust atomic primitives. Mainline accesses are
//! protected by briefly masking interrupts; the sole unmasked writer is the
//! SysTick handler.

use core::cell::UnsafeCell;

use super::dispatch::{self, Backend};
use super::flags::PeriodicSchedulerFlags;
use crate::scheduler_countdowns::{
    self, CountdownState, Features, ScanDirection, SchedulerFunction, TickContext,
};

/// Application effects driven by the live target scheduler.
pub trait ApplicationEffects {
    fn update(&mut self);
    fn timeslice_10ms(&mut self);
    fn timeslice_500ms(&mut self);
}

struct InterruptState {
    flags: PeriodicSchedulerFlags,
    countdowns: CountdownState,
    context: TickContext,
}

struct SharedState(UnsafeCell<InterruptState>);

// SAFETY: The only unmasked access is `on_systick`, whose caller must be the
// SysTick handler. Every mainline access holds `InterruptMask`, preventing the
// handler from running while an access through the cell is live.
unsafe impl Sync for SharedState {}

static STATE: SharedState = SharedState(UnsafeCell::new(InterruptState {
    flags: PeriodicSchedulerFlags::new(),
    countdowns: CountdownState::new(),
    context: TickContext {
        features: Features {
            enable_noaa: false,
            enable_voice: false,
            enable_fmradio: false,
            enable_vox: false,
        },
        function: SchedulerFunction::Foreground,
        scan_direction: ScanDirection::Off,
        css_background_scan: false,
        dual_watch_enabled: false,
        is_noaa_mode: false,
        fm_scan_active: false,
    },
}));

/// Advances the scheduler once from the SysTick exception.
///
/// # Safety
///
/// Call only from the installed SysTick handler. The single-core scheduler
/// state is otherwise accessed by mainline code under `InterruptMask`.
#[inline(never)]
pub unsafe fn on_systick() {
    // SAFETY: Guaranteed by this function's caller contract. The mainline
    // cannot execute while the SysTick exception is active.
    unsafe {
        let state = &mut *STATE.0.get();
        state.flags.tick();
        let context = state.context;
        scheduler_countdowns::execute_countdowns(&mut state.countdowns, context);
    };
}

pub fn counter() -> u32 {
    with_state(|state| state.flags.counter())
}

pub fn pending_10ms() -> bool {
    with_state(|state| state.flags.pending_10ms())
}

pub fn acknowledge_10ms() {
    with_state(|state| state.flags.acknowledge_10ms());
}

fn take_pending_10ms() -> bool {
    with_state(|state| {
        let pending = state.flags.pending_10ms();
        if pending {
            state.flags.acknowledge_10ms();
        }
        pending
    })
}

pub fn pending_40ms() -> bool {
    with_state(|state| state.flags.pending_40ms())
}

pub fn acknowledge_40ms() {
    with_state(|state| state.flags.acknowledge_40ms());
}

pub fn pending_500ms() -> bool {
    with_state(|state| state.flags.pending_500ms())
}

pub fn acknowledge_500ms() {
    with_state(|state| state.flags.acknowledge_500ms());
}

fn take_pending_500ms() -> bool {
    with_state(|state| {
        let pending = state.flags.pending_500ms();
        if pending {
            state.flags.acknowledge_500ms();
        }
        pending
    })
}

/// Cancels the boot wait without writing back a potentially stale countdown snapshot.
pub fn cancel_boot_counter() {
    with_state(|state| state.countdowns.boot_counter_10ms = 0);
}

pub fn set_tick_context(context: TickContext) {
    with_state(|state| state.context = context);
}

/// Runs one main-loop iteration against the flags produced by SysTick.
pub fn run_iteration(effects: &mut impl ApplicationEffects) {
    dispatch::run(&mut InterruptBackend { effects });
}

fn with_state<T>(operation: impl FnOnce(&mut InterruptState) -> T) -> T {
    let _interrupt_mask = InterruptMask::acquire();
    // SAFETY: Holding the mask prevents the only concurrent writer from
    // running until the borrow ends.
    unsafe { operation(&mut *STATE.0.get()) }
}

struct InterruptMask(u32);

impl InterruptMask {
    fn acquire() -> Self {
        let previous_mask: u32;
        // SAFETY: PRIMASK is available in privileged Cortex-M0 thread mode.
        // Restoring its previous value preserves an already-masked caller.
        unsafe {
            core::arch::asm!(
                "mrs {previous}, PRIMASK",
                "cpsid i",
                previous = out(reg) previous_mask,
                options(nostack, preserves_flags),
            );
        }
        Self(previous_mask)
    }
}

impl Drop for InterruptMask {
    fn drop(&mut self) {
        // SAFETY: Restore exactly the interrupt mask observed by `acquire`.
        unsafe {
            core::arch::asm!(
                "msr PRIMASK, {previous}",
                previous = in(reg) self.0,
                options(nostack, preserves_flags),
            );
        }
    }
}

struct InterruptBackend<'a, E> {
    effects: &'a mut E,
}

impl<E: ApplicationEffects> Backend for InterruptBackend<'_, E> {
    fn update(&mut self) {
        self.effects.update();
    }

    fn take_10ms_pending(&mut self) -> bool {
        take_pending_10ms()
    }

    fn timeslice_10ms(&mut self) {
        self.effects.timeslice_10ms();
    }

    fn take_500ms_pending(&mut self) -> bool {
        take_pending_500ms()
    }

    fn timeslice_500ms(&mut self) {
        self.effects.timeslice_500ms();
    }
}
