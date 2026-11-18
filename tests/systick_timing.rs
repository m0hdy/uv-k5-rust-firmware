#[path = "../src/systick_timing.rs"]
mod systick_timing;

use systick_timing::{
    SYS_TICK_MULTIPLIER, SYS_TICK_PERIOD_TICKS, delay_target_ticks, elapsed_after_sample,
    reload_value,
};

#[test]
fn reload_conversion_matches_the_pinned_period() {
    assert_eq!(SYS_TICK_PERIOD_TICKS, 480_000);
    assert_eq!(SYS_TICK_MULTIPLIER, 48);
    assert_eq!(reload_value(SYS_TICK_PERIOD_TICKS), Some(479_999));
    assert_eq!(reload_value(1), Some(0));
    assert_eq!(reload_value(0x0100_0000), Some(0x00ff_ffff));
    assert_eq!(reload_value(0), None);
    assert_eq!(reload_value(0x0100_0001), None);
}

#[test]
fn delay_target_matches_zero_and_u32_overflow_behavior() {
    assert_eq!(delay_target_ticks(0), 0);
    assert_eq!(delay_target_ticks(1), 48);
    assert_eq!(delay_target_ticks(10_000), 480_000);
    assert_eq!(delay_target_ticks(89_478_485), 4_294_967_280);
    assert_eq!(delay_target_ticks(89_478_486), 32);
}

#[test]
fn elapsed_arithmetic_matches_countdown_and_wrap_boundaries() {
    let reload = 479_999;

    assert_eq!(elapsed_after_sample(0, reload, 200, 197), Some(3));
    assert_eq!(elapsed_after_sample(0, reload, 2, reload - 1), Some(3));
    assert_eq!(elapsed_after_sample(0, reload, 2, reload), Some(2));
    assert_eq!(elapsed_after_sample(0, reload, 0, reload), Some(0));
    assert_eq!(
        elapsed_after_sample(10, reload, reload, reload - 1),
        Some(11)
    );
}

#[test]
fn elapsed_arithmetic_preserves_u32_accumulator_wrapping() {
    assert_eq!(elapsed_after_sample(u32::MAX - 1, 479_999, 10, 8), Some(0));
    assert_eq!(elapsed_after_sample(0, 479_999, 10, 10), None);
}

#[test]
fn elapsed_arithmetic_rejects_counter_values_outside_the_reload_range() {
    assert_eq!(elapsed_after_sample(0, 0x0100_0000, 1, 0), None);
    assert_eq!(elapsed_after_sample(0, 10, 11, 9), None);
    assert_eq!(elapsed_after_sample(0, 10, 9, 11), None);
}
