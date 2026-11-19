#[path = "../src/uart_init_plan.rs"]
mod uart_init_plan;

use uart_init_plan::{Register, UartInitPlan, UartInitSnapshot, UartWrite, WriteKind};

const SIGN_MASK: u32 = 1 << 31;
const DELTA_SHIFT: u32 = 11;
const MAX_DELTA: u32 = (1 << 20) - 1;

fn snapshot(delta: u32, positive: bool, uart1_ctrl: u32) -> UartInitSnapshot {
    UartInitSnapshot {
        syscon_rc_freq_delta: (delta << DELTA_SHIFT) | if positive { SIGN_MASK } else { 0 },
        uart1_ctrl,
    }
}

fn baud(delta: u32, positive: bool) -> u32 {
    UartInitPlan::from_snapshot(snapshot(delta, positive, 0))
        .writes()
        .nth(1)
        .unwrap()
        .value
}

#[test]
fn trace_has_the_seven_literal_writes_in_source_order() {
    let writes: Vec<UartWrite> = UartInitPlan::from_snapshot(snapshot(0x123, false, 0xa5a5_5a5f))
        .writes()
        .collect();

    assert_eq!(
        writes,
        vec![
            UartWrite {
                register: Register::Ctrl,
                kind: WriteKind::Modify {
                    clear_mask: 0x0000_0001,
                    set_bits: 0,
                },
                value: 0xa5a5_5a5e,
            },
            UartWrite {
                register: Register::Baud,
                kind: WriteKind::Replace,
                value: 1229,
            },
            UartWrite {
                register: Register::Ctrl,
                kind: WriteKind::Replace,
                value: 0x0000_000e,
            },
            UartWrite {
                register: Register::RxTimeout,
                kind: WriteKind::Replace,
                value: 4,
            },
            UartWrite {
                register: Register::FlowControl,
                kind: WriteKind::Replace,
                value: 0,
            },
            UartWrite {
                register: Register::Fifo,
                kind: WriteKind::Replace,
                value: 0x0000_00c7,
            },
            UartWrite {
                register: Register::InterruptEnable,
                kind: WriteKind::Replace,
                value: 0,
            },
        ]
    );
}

#[test]
fn zero_delta_and_each_sign_encode_the_expected_frequency_range() {
    assert_eq!(baud(0, false), 1229);
    assert_eq!(baud(0, true), 1229);
    assert_eq!(baud(40_000, false), 1228);
    assert_eq!(baud(40_000, true), 1230);
}

#[test]
fn maximum_twenty_bit_delta_is_used_for_both_signs() {
    assert_eq!(baud(MAX_DELTA, false), 1202);
    assert_eq!(baud(MAX_DELTA, true), 1255);
}

#[test]
fn baud_division_truncates_on_both_sides_of_divider_boundaries() {
    assert_eq!(baud(3_863, false), 1229);
    assert_eq!(baud(3_864, false), 1228);
    assert_eq!(baud(35_189, true), 1229);
    assert_eq!(baud(35_190, true), 1230);
}

#[test]
fn first_ctrl_write_preserves_unrelated_bits_before_full_replacement() {
    let writes: Vec<_> = UartInitPlan::from_snapshot(snapshot(0, false, 0xa5a5_5a5f))
        .writes()
        .collect();

    assert_eq!(writes[0].value, 0xa5a5_5a5e);
    assert_eq!(writes[0].value & !1, 0xa5a5_5a5f & !1);
    assert_eq!(writes[2].value, 0x0000_000e);
}
