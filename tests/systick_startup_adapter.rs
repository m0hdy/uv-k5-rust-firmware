#[path = "../src/systick_startup_adapter.rs"]
mod systick_startup_adapter;

use systick_startup_adapter::{
    InvalidReload, RegisterOperation, SYS_TICK_DELAY_MULTIPLIER, SYS_TICK_PERIOD_TICKS,
    apply_with, startup_sequence,
};

#[test]
fn default_startup_matches_the_pinned_cmsis_sequence() {
    assert_eq!(SYS_TICK_PERIOD_TICKS, 480_000);
    assert_eq!(SYS_TICK_DELAY_MULTIPLIER, 48);
    assert_eq!(
        startup_sequence(SYS_TICK_PERIOD_TICKS).unwrap().operations,
        [
            RegisterOperation::WriteSysTickLoad(479_999),
            RegisterOperation::UpdateSysTickPriorityWord {
                mask: 0xff00_0000,
                value: 0xc000_0000,
            },
            RegisterOperation::WriteSysTickValue(0),
            RegisterOperation::WriteSysTickControl(0b111),
        ]
    );
}

#[test]
fn minimum_and_maximum_reload_periods_are_accepted() {
    assert_eq!(
        startup_sequence(1).unwrap().operations[0],
        RegisterOperation::WriteSysTickLoad(0)
    );
    assert_eq!(
        startup_sequence(0x0100_0000).unwrap().operations[0],
        RegisterOperation::WriteSysTickLoad(0x00ff_ffff)
    );
}

#[test]
fn invalid_reload_endpoints_are_rejected_without_writes() {
    for period_ticks in [0, 0x0100_0001] {
        let mut writes = Vec::new();
        assert_eq!(
            apply_with(period_ticks, |operation| writes.push(operation)),
            Err(InvalidReload)
        );
        assert!(writes.is_empty());
    }
}

#[test]
fn priority_word_update_preserves_neighboring_exception_priorities() {
    let original = 0x5a12_3456;
    let RegisterOperation::UpdateSysTickPriorityWord { mask, value } =
        startup_sequence(SYS_TICK_PERIOD_TICKS).unwrap().operations[1]
    else {
        panic!("priority update must be the second operation");
    };

    let updated = (original & !mask) | (value & mask);
    assert_eq!(updated, 0xc012_3456);
    assert_eq!(updated & !mask, original & !mask);
}
