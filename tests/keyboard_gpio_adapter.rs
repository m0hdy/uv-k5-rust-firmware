#[path = "../src/i2c_bitbang_flow.rs"]
pub mod i2c_bitbang_flow;
#[path = "../src/keyboard_gpio_adapter.rs"]
pub mod keyboard_gpio_adapter;
#[path = "../src/keyboard_poll_flow.rs"]
pub mod keyboard_poll_flow;

use i2c_bitbang_flow::target_gpio::{Delay1us, Register, RegisterIo};
use keyboard_gpio_adapter::KeyboardGpioAdapter;
use keyboard_poll_flow::{Key, KeyEvent, KeypadEffects};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Event {
    Read(Register, u32),
    Write(Register, u32),
    Delay,
    CancelBoot,
    KeyEvent(KeyEvent),
}

#[derive(Default)]
struct State {
    values: [u32; 4],
    input_samples: VecDeque<u16>,
    sample_pending: bool,
    trace: Vec<Event>,
}

impl State {
    fn index(register: Register) -> usize {
        match register {
            Register::GpioAData => 0,
            Register::GpioADirection => 1,
            Register::PortAInputEnable => 2,
            Register::PortAOpenDrain => 3,
        }
    }
}

struct MockRegisters(Rc<RefCell<State>>);

impl RegisterIo for MockRegisters {
    fn read(&mut self, register: Register) -> u32 {
        let mut state = self.0.borrow_mut();
        let index = State::index(register);
        let mut value = state.values[index];
        if register == Register::GpioAData && state.sample_pending {
            if let Some(sample) = state.input_samples.pop_front() {
                value = (value & !0x78) | u32::from(sample & 0x78);
            }
            state.sample_pending = false;
        }
        state.trace.push(Event::Read(register, value));
        value
    }

    fn write(&mut self, register: Register, value: u32) {
        let mut state = self.0.borrow_mut();
        state.values[State::index(register)] = value;
        state.trace.push(Event::Write(register, value));
    }
}

struct MockDelay(Rc<RefCell<State>>);

impl Delay1us for MockDelay {
    fn delay_1us(&mut self) {
        let mut state = self.0.borrow_mut();
        state.sample_pending = true;
        state.trace.push(Event::Delay);
    }
}

struct Effects(Rc<RefCell<State>>);

impl KeypadEffects for Effects {
    fn cancel_boot_counter(&mut self) {
        self.0.borrow_mut().trace.push(Event::CancelBoot);
    }

    fn key_event(&mut self, event: KeyEvent) {
        self.0.borrow_mut().trace.push(Event::KeyEvent(event));
    }
}

struct Expected {
    data: u32,
    events: Vec<Event>,
}

impl Expected {
    fn new(data: u32) -> Self {
        Self {
            data,
            events: Vec::new(),
        }
    }

    fn update(&mut self, and_mask: u32, or_mask: u32) {
        self.read(Register::GpioAData, self.data);
        self.write(
            Register::GpioAData,
            (self.data & (and_mask | 0xffff_0000)) | or_mask,
        );
    }

    fn set_pin(&mut self, pin: u32, set: bool) {
        let value = if set {
            self.data | pin
        } else {
            self.data & !pin
        };
        self.read(Register::GpioAData, self.data);
        self.write(Register::GpioAData, value);
    }

    fn read(&mut self, register: Register, value: u32) {
        self.events.push(Event::Read(register, value));
    }

    fn write(&mut self, register: Register, value: u32) {
        self.data = value;
        self.events.push(Event::Write(register, value));
    }

    fn delay(&mut self) {
        self.events.push(Event::Delay);
    }

    fn scan(&mut self, rows: &[Vec<u32>]) {
        const SELECT: [u32; 5] = [0xffff, 0xfbff, 0xf7ff, 0xefff, 0xdfff];
        for (row, samples) in rows.iter().enumerate() {
            self.update(u32::MAX, 0x3c00);
            self.update(SELECT[row], 0);
            for &sample in samples {
                self.delay();
                self.read(
                    Register::GpioAData,
                    (self.data & !0x78) | u32::from(sample as u16 & 0x78),
                );
            }
        }
    }

    fn cleanup(&mut self) {
        self.set_pin(1 << 11, false);
        self.delay();
        self.set_pin(1 << 10, false);
        self.delay();
        self.set_pin(1 << 10, true);
        self.delay();
        self.set_pin(1 << 11, true);
        self.delay();
        self.update(0xffff_efff, 0);
        self.update(u32::MAX, 0x2000);
    }
}

fn poll(
    adapter: &mut KeyboardGpioAdapter<MockRegisters, MockDelay>,
    state: &Rc<RefCell<State>>,
    rows: &[Vec<u32>],
    effects: &[Event],
) {
    let start = state.borrow().trace.len();
    let mut expected = Expected::new(state.borrow().values[0]);
    expected.scan(rows);
    expected.cleanup();
    expected.events.extend_from_slice(effects);

    state.borrow_mut().input_samples = rows.iter().flatten().map(|sample| *sample as u16).collect();

    adapter.poll(&mut Effects(Rc::clone(state)));

    let shared = state.borrow();
    assert!(
        shared.input_samples.is_empty(),
        "poll omitted expected GPIOA input samples"
    );
    assert_eq!(&shared.trace[start..], expected.events);
}

fn adapter() -> (
    KeyboardGpioAdapter<MockRegisters, MockDelay>,
    Rc<RefCell<State>>,
) {
    let state = Rc::new(RefCell::new(State {
        values: [0xa5a5_c3a5, 0x1234_5678, 0x8765_4321, 0x7654_3210],
        ..State::default()
    }));
    (
        KeyboardGpioAdapter::new(
            MockRegisters(Rc::clone(&state)),
            MockDelay(Rc::clone(&state)),
        ),
        state,
    )
}

fn samples_for_key(row: usize, column: usize) -> Vec<Vec<u32>> {
    const PINS: [[usize; 4]; 5] = [
        [3, 4, 4, 4],
        [3, 4, 5, 6],
        [3, 4, 5, 6],
        [3, 4, 5, 6],
        [3, 4, 5, 6],
    ];
    let mut rows = vec![vec![0xffff; 3]; row];
    rows.push(vec![0xffff & !(1 << PINS[row][column]); 3]);
    rows
}

fn all_rows_no_key() -> Vec<Vec<u32>> {
    vec![vec![0xffff; 3]; 5]
}

#[test]
fn all_side_and_matrix_keys_poll_through_the_register_backend() {
    let keys = [
        [Some(Key::Side1), Some(Key::Side2), None, None],
        [
            Some(Key::Menu),
            Some(Key::Digit1),
            Some(Key::Digit4),
            Some(Key::Digit7),
        ],
        [
            Some(Key::Up),
            Some(Key::Digit2),
            Some(Key::Digit5),
            Some(Key::Digit8),
        ],
        [
            Some(Key::Down),
            Some(Key::Digit3),
            Some(Key::Digit6),
            Some(Key::Digit9),
        ],
        [
            Some(Key::Exit),
            Some(Key::Star),
            Some(Key::Digit0),
            Some(Key::F),
        ],
    ];

    for (row, keys) in keys.iter().enumerate() {
        for (column, key) in keys.iter().enumerate() {
            let Some(key) = key else { continue };
            let (mut adapter, state) = adapter();
            let samples = samples_for_key(row, column);
            poll(&mut adapter, &state, &samples, &[Event::CancelBoot]);
            poll(&mut adapter, &state, &samples, &[Event::CancelBoot]);
            poll(
                &mut adapter,
                &state,
                &samples,
                &[Event::CancelBoot, Event::KeyEvent(KeyEvent::Pressed(*key))],
            );
        }
    }
}

#[test]
fn first_side_key_wins_and_every_poll_has_exact_stop_and_voice_cleanup() {
    let (mut adapter, state) = adapter();
    let both_side_keys_low = [vec![0xffe7; 3]];
    poll(
        &mut adapter,
        &state,
        &both_side_keys_low,
        &[Event::CancelBoot],
    );
    poll(
        &mut adapter,
        &state,
        &both_side_keys_low,
        &[Event::CancelBoot],
    );
    poll(
        &mut adapter,
        &state,
        &both_side_keys_low,
        &[
            Event::CancelBoot,
            Event::KeyEvent(KeyEvent::Pressed(Key::Side1)),
        ],
    );
}

#[test]
fn zero_sample_three_stable_and_eight_attempt_boundaries_are_actual_reads() {
    let (mut adapter, state) = adapter();
    poll(&mut adapter, &state, &[vec![0, 0, 0]], &[Event::CancelBoot]);

    let late_stable = [vec![0x08, 0x10, 0x18, 0x20, 0x30, 0x70, 0x70, 0x70]];
    poll(&mut adapter, &state, &late_stable, &[Event::CancelBoot]);

    let two_at_limit = [vec![0x08, 0x10, 0x18, 0x20, 0x30, 0x38, 0x70, 0x70]];
    poll(&mut adapter, &state, &two_at_limit, &[]);
}

#[test]
fn no_key_and_noise_abort_both_run_shared_stop_and_voice_cleanup() {
    let (mut adapter, state) = adapter();
    poll(&mut adapter, &state, &all_rows_no_key(), &[]);

    let noise = [(0..8).map(|i| 0x70 ^ ((i % 2) << 3)).collect()];
    poll(&mut adapter, &state, &noise, &[]);
}

#[test]
fn actual_register_trace_preserves_upper_bits_and_cleans_up_early_key() {
    let (mut adapter, state) = adapter();
    state.borrow_mut().input_samples = [0xfff7; 3].into();
    adapter.poll(&mut Effects(Rc::clone(&state)));

    let shared = state.borrow();
    let data_accesses: Vec<_> = shared
        .trace
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::Read(Register::GpioAData, _) | Event::Write(Register::GpioAData, _)
            )
        })
        .take(4)
        .cloned()
        .collect();
    assert_eq!(
        data_accesses,
        [
            Event::Read(Register::GpioAData, 0xa5a5_c3a5),
            Event::Write(Register::GpioAData, 0xa5a5_ffa5),
            Event::Read(Register::GpioAData, 0xa5a5_ffa5),
            Event::Write(Register::GpioAData, 0xa5a5_ffa5),
        ]
    );
    let data_writes: Vec<_> = shared
        .trace
        .iter()
        .filter_map(|event| match event {
            Event::Write(Register::GpioAData, value) => Some(*value),
            _ => None,
        })
        .collect();
    assert_eq!(
        data_writes,
        [
            0xa5a5_ffa5,
            0xa5a5_ffa5,
            0xa5a5_f7a5,
            0xa5a5_f3a5,
            0xa5a5_f7a5,
            0xa5a5_ffa5,
            0xa5a5_efa5,
            0xa5a5_efa5,
        ]
    );
    let samples: Vec<_> = shared
        .trace
        .iter()
        .filter_map(|event| match event {
            Event::Read(Register::GpioAData, value) if value & 0x78 == 0x70 => Some(*value),
            _ => None,
        })
        .collect();
    assert_eq!(samples, [0xa5a5_fff5; 3]);
    assert!(shared.input_samples.is_empty());
    assert!(data_writes.iter().all(|value| value >> 16 == 0xa5a5));
    assert_eq!(
        &shared.trace[shared.trace.len() - 3..],
        [
            Event::Read(Register::GpioAData, 0xa5a5_efa5),
            Event::Write(Register::GpioAData, 0xa5a5_efa5),
            Event::CancelBoot,
        ]
    );
}

#[test]
fn retained_debounce_releases_old_key_on_direct_change_before_new_press() {
    let (mut adapter, state) = adapter();
    let first = samples_for_key(1, 1);
    poll(&mut adapter, &state, &first, &[Event::CancelBoot]);
    poll(&mut adapter, &state, &first, &[Event::CancelBoot]);
    poll(
        &mut adapter,
        &state,
        &first,
        &[
            Event::CancelBoot,
            Event::KeyEvent(KeyEvent::Pressed(Key::Digit1)),
        ],
    );

    let second = samples_for_key(2, 1);
    poll(
        &mut adapter,
        &state,
        &second,
        &[
            Event::CancelBoot,
            Event::KeyEvent(KeyEvent::Released {
                key: Key::Digit1,
                was_held: false,
            }),
        ],
    );
    poll(&mut adapter, &state, &second, &[Event::CancelBoot]);
    poll(
        &mut adapter,
        &state,
        &second,
        &[
            Event::CancelBoot,
            Event::KeyEvent(KeyEvent::Pressed(Key::Digit2)),
        ],
    );
}
