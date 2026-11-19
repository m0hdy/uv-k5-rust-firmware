#[path = "../src/app_keypad_memory_flow.rs"]
mod app_keypad_memory_flow;

use app_keypad_memory_flow::{
    Digit, FlowEffect, Key, KeyEvent, KeypadMemoryEffects, KeypadMemoryFlow, MatrixIo,
    MemoryChannelContext,
};
use std::collections::VecDeque;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Op {
    Update(u32, u32),
    Read,
    Delay,
    Stop,
    CancelBoot,
    Memory(FlowEffect),
}

struct MockMatrix {
    readings: VecDeque<u32>,
    trace: Vec<Op>,
}

impl MatrixIo for MockMatrix {
    fn update_gpioa(&mut self, and_mask: u32, or_mask: u32) {
        self.trace.push(Op::Update(and_mask, or_mask));
    }

    fn read_gpioa(&mut self) -> u32 {
        self.trace.push(Op::Read);
        self.readings.pop_front().expect("unexpected matrix read")
    }

    fn delay_1us(&mut self) {
        self.trace.push(Op::Delay);
    }

    fn i2c_stop(&mut self) {
        self.trace.push(Op::Stop);
    }
}

impl KeypadMemoryEffects for Vec<Op> {
    fn cancel_boot_counter(&mut self) {
        self.push(Op::CancelBoot);
    }

    fn memory_effect(&mut self, effect: FlowEffect) {
        self.push(Op::Memory(effect));
    }
}

const CONTEXT: MemoryChannelContext = MemoryChannelContext {
    memory_mode: true,
    f_key_pressed: false,
    is_active_tx_vfo: true,
    selected_vfo: 1,
};

const VALID_007_EFFECTS: [FlowEffect; 9] = [
    FlowEffect::RefreshKeyInputTimeout,
    FlowEffect::AppendDigit(Digit::Seven),
    FlowEffect::RequestMainDisplay,
    FlowEffect::ResetInputIndex,
    FlowEffect::CheckChannelValidity(6),
    FlowEffect::UpdateSelectedVfoMemoryChannel { vfo: 1, channel: 6 },
    FlowEffect::UpdateSelectedVfoScreenChannel { vfo: 1, channel: 6 },
    FlowEffect::RequestSaveVfo,
    FlowEffect::ConfigureVfoReload,
];

fn rows(key: Option<(usize, usize)>) -> Vec<u32> {
    let mut readings = Vec::new();
    for row in 0..5 {
        let mut reading = 0xffff;
        if let Some((key_row, column)) = key
            && row == key_row
        {
            reading &= !(1 << (3 + column));
        }
        readings.extend([reading; 3]);
        if key.is_some_and(|(key_row, _)| row == key_row) {
            break;
        }
    }
    readings
}

fn poll(
    flow: &mut KeypadMemoryFlow,
    key: Option<(usize, usize)>,
    context: MemoryChannelContext,
    valid: impl FnMut(u16) -> bool,
) -> (Option<KeyEvent>, Vec<Op>) {
    let mut io = MockMatrix {
        readings: rows(key).into(),
        trace: Vec::new(),
    };
    let mut downstream = Vec::new();
    let event = flow.poll(&mut io, context, &mut downstream, valid);
    assert!(io.readings.is_empty(), "scan returned too soon");

    io.trace.extend(downstream);
    (event, io.trace)
}

fn cancel_and_press() -> [Op; 4] {
    [
        Op::Stop,
        Op::Update(0xffff_efff, 0),
        Op::Update(u32::MAX, 0x2000),
        Op::CancelBoot,
    ]
}

fn press_digit(flow: &mut KeypadMemoryFlow, key: (usize, usize), expected: Key) {
    for _ in 0..2 {
        let (event, trace) = poll(flow, Some(key), CONTEXT, |_| false);
        assert_eq!(event, None);
        assert_eq!(&trace[trace.len() - 4..], &cancel_and_press());
    }
    let (event, trace) = poll(flow, Some(key), CONTEXT, |_| false);
    assert_eq!(event, Some(KeyEvent::Pressed(expected)));
    assert_eq!(
        &trace[trace.len() - 5..],
        &[
            Op::Stop,
            Op::Update(0xffff_efff, 0),
            Op::Update(u32::MAX, 0x2000),
            Op::CancelBoot,
            Op::Memory(FlowEffect::RequestOptionalKeyBeep),
        ]
    );
}

fn release_digit(flow: &mut KeypadMemoryFlow, expected: Key, tail: &[FlowEffect]) {
    for _ in 0..2 {
        let (event, trace) = poll(flow, None, CONTEXT, |_| false);
        assert_eq!(event, None);
        assert_eq!(
            &trace[trace.len() - 3..],
            &[
                Op::Stop,
                Op::Update(0xffff_efff, 0),
                Op::Update(u32::MAX, 0x2000),
            ]
        );
    }
    let (event, trace) = poll(flow, None, CONTEXT, |channel| channel == 6);
    assert!(matches!(
        event,
        Some(KeyEvent::Released {
            key,
            was_held: false
        }) if key == expected
    ));
    assert_eq!(
        &trace[trace.len() - tail.len() - 3..],
        &[
            vec![
                Op::Stop,
                Op::Update(0xffff_efff, 0),
                Op::Update(u32::MAX, 0x2000),
            ],
            tail.iter().copied().map(Op::Memory).collect::<Vec<_>>(),
        ]
        .concat()
    );
}

#[test]
fn actual_matrix_polls_enter_007_and_deliver_literal_vfo_save_reload_effects() {
    let mut flow = KeypadMemoryFlow::new();
    let digit = [
        FlowEffect::RefreshKeyInputTimeout,
        FlowEffect::AppendDigit(Digit::Zero),
        FlowEffect::RequestMainDisplay,
    ];
    for (key, logical) in [
        ((4, 2), Key::Digit0),
        ((4, 2), Key::Digit0),
        ((1, 3), Key::Digit7),
    ] {
        press_digit(&mut flow, key, logical);
        let expected: &[FlowEffect] = if logical == Key::Digit7 {
            &VALID_007_EFFECTS
        } else {
            &digit
        };
        release_digit(&mut flow, logical, expected);
    }

    assert_eq!(flow.input_box().index(), 0);
    assert_eq!(flow.input_box().ascii_projection(), *b"007-----");
}

#[test]
fn invalid_entry_and_held_digit_never_select_a_channel() {
    let mut invalid = KeypadMemoryFlow::new();
    for (key, logical) in [
        ((4, 2), Key::Digit0),
        ((4, 2), Key::Digit0),
        ((4, 2), Key::Digit0),
    ] {
        press_digit(&mut invalid, key, logical);
        for _ in 0..2 {
            poll(&mut invalid, None, CONTEXT, |_| false);
        }
        let (_, effects) = poll(&mut invalid, None, CONTEXT, |_| false);
        if invalid.input_box().index() == 0 {
            assert!(effects.contains(&Op::Memory(FlowEffect::RequestOptionalInvalidChannelBeep)));
            assert!(!effects.iter().any(|effect| matches!(
                effect,
                Op::Memory(FlowEffect::UpdateSelectedVfoMemoryChannel { .. })
                    | Op::Memory(FlowEffect::UpdateSelectedVfoScreenChannel { .. })
                    | Op::Memory(FlowEffect::RequestSaveVfo)
                    | Op::Memory(FlowEffect::ConfigureVfoReload)
            )));
        }
    }

    let mut held = KeypadMemoryFlow::new();
    press_digit(&mut held, (1, 1), Key::Digit1);
    for _ in 0..37 {
        poll(&mut held, Some((1, 1)), CONTEXT, |_| {
            panic!("validity is not queried while held")
        });
    }
    let (event, trace) = poll(&mut held, Some((1, 1)), CONTEXT, |_| {
        panic!("validity is not queried on a held digit")
    });
    assert_eq!(event, Some(KeyEvent::Held(Key::Digit1)));
    assert!(!trace.iter().any(|op| matches!(
        op,
        Op::Memory(FlowEffect::UpdateSelectedVfoMemoryChannel { .. })
            | Op::Memory(FlowEffect::UpdateSelectedVfoScreenChannel { .. })
            | Op::Memory(FlowEffect::RequestSaveVfo)
            | Op::Memory(FlowEffect::ConfigureVfoReload)
    )));
    let (event, trace) = poll(&mut held, Some((1, 1)), CONTEXT, |_| {
        panic!("validity is not queried by another held poll")
    });
    assert_eq!(event, None);
    assert!(trace.contains(&Op::CancelBoot));
    assert!(!trace.iter().any(|op| matches!(op, Op::Memory(_))));
}

#[test]
fn caller_f_state_suppresses_digit_release_entry() {
    let context = MemoryChannelContext {
        f_key_pressed: true,
        ..CONTEXT
    };
    let mut flow = KeypadMemoryFlow::new();
    for _ in 0..3 {
        poll(&mut flow, Some((4, 2)), context, |_| {
            panic!("validity is not queried on a digit press")
        });
    }
    for _ in 0..2 {
        let (_, trace) = poll(&mut flow, None, context, |_| {
            panic!("validity is not queried before release debounce")
        });
        assert!(!trace.iter().any(|op| matches!(op, Op::Memory(_))));
    }
    let (event, trace) = poll(&mut flow, None, context, |_| {
        panic!("F-modified digit release must not query validity")
    });
    assert_eq!(
        event,
        Some(KeyEvent::Released {
            key: Key::Digit0,
            was_held: false
        })
    );
    assert!(!trace.iter().any(|op| matches!(op, Op::Memory(_))));
    assert_eq!(flow.input_box().index(), 0);
}

#[test]
fn memory_actions_require_eligible_mode_and_active_tx_vfo() {
    for context in [
        MemoryChannelContext {
            memory_mode: false,
            ..CONTEXT
        },
        MemoryChannelContext {
            is_active_tx_vfo: false,
            ..CONTEXT
        },
    ] {
        let mut flow = KeypadMemoryFlow::new();
        for _ in 0..3 {
            let (_, trace) = poll(&mut flow, Some((1, 1)), context, |_| {
                panic!("ineligible entry must not query validity")
            });
            assert!(trace.contains(&Op::CancelBoot));
            assert!(!trace.iter().any(|op| matches!(op, Op::Memory(_))));
        }
        for _ in 0..3 {
            let (_, trace) = poll(&mut flow, None, context, |_| {
                panic!("ineligible entry must not query validity")
            });
            assert!(!trace.iter().any(|op| matches!(op, Op::Memory(_))));
        }
    }
}

#[test]
fn cleanup_precedes_boot_cancellation_and_debounced_effects() {
    let mut flow = KeypadMemoryFlow::new();
    for _ in 0..3 {
        poll(&mut flow, Some((4, 2)), CONTEXT, |_| false);
    }
    for _ in 0..2 {
        poll(&mut flow, None, CONTEXT, |_| false);
    }
    let (_, trace) = poll(&mut flow, None, CONTEXT, |_| false);
    let cleanup = trace
        .windows(3)
        .position(|window| {
            window
                == [
                    Op::Stop,
                    Op::Update(0xffff_efff, 0),
                    Op::Update(u32::MAX, 0x2000),
                ]
        })
        .expect("scanner cleanup");
    assert_eq!(
        &trace[cleanup + 3..],
        &[
            Op::Memory(FlowEffect::RefreshKeyInputTimeout),
            Op::Memory(FlowEffect::AppendDigit(Digit::Zero)),
            Op::Memory(FlowEffect::RequestMainDisplay),
        ]
    );
}
