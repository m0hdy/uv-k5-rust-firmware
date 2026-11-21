//! Shared SysTick flags for the target main loop.
//!
//! The Cortex-M0 target has no Rust atomic primitives. Mainline accesses are
//! protected by briefly masking interrupts; the sole unmasked writer is the
//! SysTick handler.

use core::cell::UnsafeCell;

use super::flags::PeriodicSchedulerFlags;

struct SharedFlags(UnsafeCell<PeriodicSchedulerFlags>);

// SAFETY: The only unmasked access is `on_systick`, whose caller must be the
// SysTick handler. Every mainline access holds `InterruptMask`, preventing the
// handler from running while an access through the cell is live.
unsafe impl Sync for SharedFlags {}

static FLAGS: SharedFlags = SharedFlags(UnsafeCell::new(PeriodicSchedulerFlags::new()));

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
    unsafe { (&mut *FLAGS.0.get()).tick() };
}

pub fn counter() -> u32 {
    with_flags(|flags| flags.counter())
}

pub fn pending_10ms() -> bool {
    with_flags(|flags| flags.pending_10ms())
}

pub fn acknowledge_10ms() {
    with_flags(|flags| flags.acknowledge_10ms());
}

pub fn pending_40ms() -> bool {
    with_flags(|flags| flags.pending_40ms())
}

pub fn acknowledge_40ms() {
    with_flags(|flags| flags.acknowledge_40ms());
}

pub fn pending_500ms() -> bool {
    with_flags(|flags| flags.pending_500ms())
}

pub fn acknowledge_500ms() {
    with_flags(|flags| flags.acknowledge_500ms());
}

fn with_flags<T>(operation: impl FnOnce(&mut PeriodicSchedulerFlags) -> T) -> T {
    let _interrupt_mask = InterruptMask::acquire();
    // SAFETY: Holding the mask prevents the only concurrent writer from
    // running until the borrow ends.
    unsafe { operation(&mut *FLAGS.0.get()) }
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
