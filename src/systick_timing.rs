//! Pure arithmetic used by the pinned SysTick delay routine.
//!
//! The reference is `driver/systick.c` at
//! `egzumer/uv-k5-firmware-custom` revision
//! `7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`. Its setup requests 480,000
//! counter ticks, and its delay multiplier is 48; the target contract in
//! `docs/target-contract.md` notes that these values do not establish the
//! actual core frequency. `SysTick_Config` expresses a tick count, while the
//! 24-bit LOAD register stores that count minus one.
//!
//! These helpers reproduce the reference's `u32` wrapping arithmetic,
//! including its one-tick-short wrap calculation. They do not read the timer,
//! wait for it to change, configure hardware, or validate elapsed real time.

pub const SYS_TICK_PERIOD_TICKS: u32 = 480_000;
pub const SYS_TICK_MULTIPLIER: u32 = 48;
pub const SYS_TICK_LOAD_MAX: u32 = 0x00ff_ffff;
const SYS_TICK_PERIOD_MAX: u32 = SYS_TICK_LOAD_MAX + 1;

/// Converts a SysTick period in ticks to its 24-bit LOAD register value.
pub const fn reload_value(period_ticks: u32) -> Option<u32> {
    if period_ticks == 0 || period_ticks > SYS_TICK_PERIOD_MAX {
        None
    } else {
        Some(period_ticks - 1)
    }
}

/// Computes the reference delay target using its 32-bit unsigned multiplication.
pub const fn delay_target_ticks(delay_us: u32) -> u32 {
    delay_us.wrapping_mul(SYS_TICK_MULTIPLIER)
}

/// Applies one changed counter sample to the reference's accumulated elapsed ticks.
///
/// Returns `None` for a sample outside the reload range or an unchanged value;
/// the C routine waits for a changed value before applying this arithmetic.
pub const fn elapsed_after_sample(
    elapsed_ticks: u32,
    reload: u32,
    previous: u32,
    current: u32,
) -> Option<u32> {
    if reload > SYS_TICK_LOAD_MAX
        || previous > reload
        || current > reload
        || current == previous
    {
        return None;
    }

    let delta = if current < previous {
        current.wrapping_neg()
    } else {
        reload - current
    };
    Some(elapsed_ticks.wrapping_add(delta.wrapping_add(previous)))
}
