#[path = "../src/keyboard_poll_flow.rs"]
mod keyboard_poll_flow;

use keyboard_poll_flow::{Key, KeyEvent, KeyboardPollFlow, KeypadEffects, MatrixIo};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Op {
    Update(u32, u32),
    Read,
    Delay,
    Stop,
    CancelBoot,
    Event(KeyEvent),
}

struct Mock {
    readings: VecDeque<u32>,
    trace: Rc<RefCell<Vec<Op>>>,
}

impl MatrixIo for Mock {
    fn update_gpioa(&mut self, and_mask: u32, or_mask: u32) {
        self.trace.borrow_mut().push(Op::Update(and_mask, or_mask));
    }

    fn read_gpioa(&mut self) -> u32 {
        self.trace.borrow_mut().push(Op::Read);
        self.readings.pop_front().expect("unexpected matrix read")
    }

    fn delay_1us(&mut self) {
        self.trace.borrow_mut().push(Op::Delay);
    }

    fn i2c_stop(&mut self) {
        self.trace.borrow_mut().push(Op::Stop);
    }
}

struct Effects(Rc<RefCell<Vec<Op>>>);

impl KeypadEffects for Effects {
    fn cancel_boot_counter(&mut self) {
        self.0.borrow_mut().push(Op::CancelBoot);
    }

    fn key_event(&mut self, event: KeyEvent) {
        self.0.borrow_mut().push(Op::Event(event));
    }
}

fn key_rows(row: usize, column: usize) -> Vec<Vec<u32>> {
    let mut rows = vec![vec![0xffff; 3]; row];
    rows.push(vec![0xffff & !(1 << (3 + column)); 3]);
    rows
}

fn no_key() -> Vec<Vec<u32>> {
    vec![vec![0xffff; 3]; 5]
}

fn noise(row: usize) -> Vec<Vec<u32>> {
    let mut rows = vec![vec![0xffff; 3]; row];
    rows.push((0..8).map(|i| 0xfff7 ^ (i % 2)).collect());
    rows
}

fn tick(flow: &mut KeyboardPollFlow, rows: Vec<Vec<u32>>, effects: &[Op]) {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let mut io = Mock {
        readings: rows.into_iter().flatten().collect(),
        trace: Rc::clone(&trace),
    };
    flow.poll(&mut io, &mut Effects(Rc::clone(&trace)));
    assert!(io.readings.is_empty(), "scan returned too soon");

    let trace = trace.borrow();
    let stop = trace.iter().position(|op| *op == Op::Stop).unwrap();
    assert!(
        trace[..stop]
            .iter()
            .all(|op| matches!(op, Op::Update(..) | Op::Read | Op::Delay))
    );
    let mut expected = vec![
        Op::Stop,
        Op::Update(0xffff_efff, 0),
        Op::Update(u32::MAX, 0x2000),
    ];
    expected.extend_from_slice(effects);
    assert_eq!(&trace[stop..], expected);
}

fn press(flow: &mut KeyboardPollFlow, row: usize, column: usize, key: Key) {
    tick(flow, key_rows(row, column), &[Op::CancelBoot]);
    tick(flow, key_rows(row, column), &[Op::CancelBoot]);
    tick(
        flow,
        key_rows(row, column),
        &[Op::CancelBoot, Op::Event(KeyEvent::Pressed(key))],
    );
}

#[test]
fn all_matrix_keys_map_by_meaning_and_cancel_before_press() {
    let keys: &[&[Key]] = &[
        &[Key::Side1, Key::Side2],
        &[Key::Menu, Key::Digit1, Key::Digit4, Key::Digit7],
        &[Key::Up, Key::Digit2, Key::Digit5, Key::Digit8],
        &[Key::Down, Key::Digit3, Key::Digit6, Key::Digit9],
        &[Key::Exit, Key::Star, Key::Digit0, Key::F],
    ];
    for (row, keys) in keys.iter().enumerate() {
        for (column, &key) in keys.iter().enumerate() {
            press(&mut KeyboardPollFlow::new(), row, column, key);
        }
    }
}

#[test]
fn stable_release_is_debounced_without_boot_cancellation() {
    let mut flow = KeyboardPollFlow::new();
    tick(&mut flow, no_key(), &[]);
    press(&mut flow, 1, 2, Key::Digit4);
    tick(&mut flow, no_key(), &[]);
    tick(&mut flow, no_key(), &[]);
    tick(
        &mut flow,
        no_key(),
        &[Op::Event(KeyEvent::Released {
            key: Key::Digit4,
            was_held: false,
        })],
    );
}

#[test]
fn held_directional_key_repeats_and_release_remembers_hold() {
    for (row, key) in [(2, Key::Up), (3, Key::Down)] {
        let mut flow = KeyboardPollFlow::new();
        press(&mut flow, row, 0, key);
        for _ in 0..37 {
            tick(&mut flow, key_rows(row, 0), &[Op::CancelBoot]);
        }
        tick(
            &mut flow,
            key_rows(row, 0),
            &[Op::CancelBoot, Op::Event(KeyEvent::Held(key))],
        );
        for _ in 0..7 {
            tick(&mut flow, key_rows(row, 0), &[Op::CancelBoot]);
        }
        tick(
            &mut flow,
            key_rows(row, 0),
            &[Op::CancelBoot, Op::Event(KeyEvent::Held(key))],
        );
        tick(&mut flow, no_key(), &[]);
        tick(&mut flow, no_key(), &[]);
        tick(
            &mut flow,
            no_key(),
            &[Op::Event(KeyEvent::Released {
                key,
                was_held: true,
            })],
        );
    }
}

#[test]
fn direct_switch_cancels_boot_before_releasing_previous_key() {
    let mut flow = KeyboardPollFlow::new();
    press(&mut flow, 1, 1, Key::Digit1);
    tick(
        &mut flow,
        key_rows(2, 1),
        &[
            Op::CancelBoot,
            Op::Event(KeyEvent::Released {
                key: Key::Digit1,
                was_held: false,
            }),
        ],
    );
    tick(&mut flow, key_rows(2, 1), &[Op::CancelBoot]);
    tick(
        &mut flow,
        key_rows(2, 1),
        &[Op::CancelBoot, Op::Event(KeyEvent::Pressed(Key::Digit2))],
    );
}

#[test]
fn noise_is_no_key_and_still_cleans_up_before_release() {
    for row in 0..5 {
        let mut flow = KeyboardPollFlow::new();
        tick(&mut flow, noise(row), &[]);
        press(&mut flow, 0, 0, Key::Side1);
        tick(&mut flow, noise(row), &[]);
        tick(&mut flow, noise(row), &[]);
        tick(
            &mut flow,
            noise(row),
            &[Op::Event(KeyEvent::Released {
                key: Key::Side1,
                was_held: false,
            })],
        );
    }
}
