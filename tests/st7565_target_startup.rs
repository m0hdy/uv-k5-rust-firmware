use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::num::NonZeroU32;
use std::rc::Rc;

use uv_k5_rust_firmware::st7565_line_transport::{DRAIN_POLL_LIMIT, LineError};
use uv_k5_rust_firmware::st7565_startup_executor::target_spi0::{
    DelayMs, RegisterIo, startup_with_io,
};

const CR: usize = 0x400b_8000;
const WDR: usize = 0x400b_8004;
const IE: usize = 0x400b_8010;
const IF: usize = 0x400b_8014;
const FIFOST: usize = 0x400b_8018;
const GPIOB: usize = 0x4006_0800;
const CLOCK: usize = 0x4000_0008;
const MODE: u32 = 1 << 12;
const A0: u32 = 1 << 9;
const RESET: u32 = 1 << 11;
const STARTUP_COMMANDS: [u8; 17] = [
    0xe2, 0xa2, 0xc0, 0xa1, 0xa6, 0xa4, 0x24, 0x81, 0x1f, 0x2b, 0x2e, 0x2f, 0x2f, 0x2f, 0x2f, 0x40,
    0xaf,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Read(usize, u32),
    Write(usize, u32),
    Delay(u16),
}

type Trace = Rc<RefCell<Vec<Event>>>;

struct MockRegisters {
    values: BTreeMap<usize, u32>,
    scripted: BTreeMap<usize, VecDeque<u32>>,
    trace: Trace,
}

impl MockRegisters {
    fn ready(trace: Trace) -> Self {
        Self {
            values: [
                (CR, 0xa5a5_ffff),
                (WDR, 0),
                (IE, u32::MAX),
                (IF, 0),
                (FIFOST, 0),
                (GPIOB, 0x5a5a_f7ff),
                (CLOCK, 0x1234_0000),
            ]
            .into(),
            scripted: BTreeMap::new(),
            trace,
        }
    }

    fn script(&mut self, address: usize, values: impl IntoIterator<Item = u32>) {
        self.scripted.insert(address, values.into_iter().collect());
    }

    fn bytes(&self) -> Vec<u8> {
        self.trace
            .borrow()
            .iter()
            .filter_map(|event| match event {
                Event::Write(WDR, byte) => Some(*byte as u8),
                _ => None,
            })
            .collect()
    }

    fn reads(&self, address: usize) -> usize {
        self.trace
            .borrow()
            .iter()
            .filter(|event| matches!(event, Event::Read(a, _) if *a == address))
            .count()
    }
}

impl RegisterIo for MockRegisters {
    fn read(&mut self, address: usize) -> u32 {
        assert_eq!(address % 4, 0);
        let value = self
            .scripted
            .get_mut(&address)
            .and_then(VecDeque::pop_front)
            .unwrap_or_else(|| *self.values.get(&address).expect("known physical register"));
        self.trace.borrow_mut().push(Event::Read(address, value));
        value
    }

    fn write(&mut self, address: usize, value: u32) {
        assert_eq!(address % 4, 0);
        *self
            .values
            .get_mut(&address)
            .expect("known physical register") = value;
        self.trace.borrow_mut().push(Event::Write(address, value));
    }
}

struct MockDelay(Trace);

impl DelayMs for MockDelay {
    fn delay_ms(&mut self, milliseconds: u16) {
        self.0.borrow_mut().push(Event::Delay(milliseconds));
    }
}

fn mocks() -> (MockRegisters, MockDelay) {
    let trace = Trace::default();
    (MockRegisters::ready(trace.clone()), MockDelay(trace))
}

fn run(io: &mut MockRegisters, delay: &mut MockDelay) -> Result<(), LineError> {
    startup_with_io(io, delay, NonZeroU32::new(3).unwrap())
}

fn rmw(expected: &mut Vec<Event>, address: usize, current: &mut u32, updated: u32) {
    expected.extend([
        Event::Read(address, *current),
        Event::Write(address, updated),
    ]);
    *current = updated;
}

#[test]
fn exact_physical_trace_initializes_before_reset_and_selects_eight_pages_without_pixels() {
    let (mut io, mut delay) = mocks();
    assert_eq!(run(&mut io, &mut delay), Ok(()));

    let mut expected = vec![
        Event::Read(CR, 0xa5a5_ffff),
        Event::Write(CR, 0xa5a5_fff7),
        Event::Read(CLOCK, 0x1234_0000),
        Event::Write(CLOCK, 0x1234_0400),
        Event::Read(CR, 0xa5a5_fff7),
        Event::Write(CR, 0xa5a5_fff7),
        Event::Read(CR, 0xa5a5_fff7),
        Event::Write(CR, 0xa5a5_7f72),
        Event::Write(IE, 0),
        Event::Read(IE, 0),
        Event::Read(CR, 0xa5a5_7f72),
        Event::Write(CR, 0xa5a5_7f7a),
    ];
    let mut gpio = 0x5a5a_f7ff;
    for (high, milliseconds) in [(true, 1), (false, 20), (true, 120)] {
        let updated = if high { gpio | RESET } else { gpio & !RESET };
        rmw(&mut expected, GPIOB, &mut gpio, updated);
        expected.push(Event::Delay(milliseconds));
    }
    let mut control = 0xa5a5_7f7a;
    let updated = control & !MODE;
    rmw(&mut expected, CR, &mut control, updated);
    for (index, byte) in STARTUP_COMMANDS.into_iter().enumerate() {
        let updated = gpio & !A0;
        rmw(&mut expected, GPIOB, &mut gpio, updated);
        expected.extend([Event::Read(FIFOST, 0), Event::Write(WDR, byte.into())]);
        match index {
            0 => expected.push(Event::Delay(120)),
            9 | 10 => expected.push(Event::Delay(1)),
            14 => expected.push(Event::Delay(40)),
            _ => {}
        }
    }
    expected.push(Event::Read(IF, 0));
    let updated = control | MODE;
    rmw(&mut expected, CR, &mut control, updated);
    let updated = control & !MODE;
    rmw(&mut expected, CR, &mut control, updated);
    for page in 0..8 {
        let updated = gpio & !A0;
        rmw(&mut expected, GPIOB, &mut gpio, updated);
        for byte in [0xb0 + page, 0x10, 0x04] {
            expected.extend([Event::Read(FIFOST, 0), Event::Write(WDR, byte)]);
        }
        expected.push(Event::Read(IF, 0));
        let updated = gpio | A0;
        rmw(&mut expected, GPIOB, &mut gpio, updated);
        expected.push(Event::Read(IF, 0));
    }
    let updated = control | MODE;
    rmw(&mut expected, CR, &mut control, updated);
    assert_eq!(*io.trace.borrow(), expected);
    assert_eq!(io.bytes().len(), 17 + 8 * 3);
    assert_eq!(
        io.values[&GPIOB] & !(A0 | RESET),
        0x5a5a_f7ff & !(A0 | RESET)
    );
    assert_eq!(io.values[&CR] & 0xffff_0000, 0xa5a5_0000);
}

#[test]
fn every_shared_register_rmw_uses_a_fresh_snapshot() {
    let (mut io, mut delay) = mocks();
    io.script(
        CR,
        [
            0xa5a5_ffff,
            0x1357_9bff,
            0x2469_1ff7,
            0xdead_0000,
            0xaaaa_ffff,
            0xbbbb_0000,
            0xcccc_ffff,
            0xdddd_0000,
        ],
    );
    // Alternate unrelated bits on *every* read, not only between phases.
    let gpio_snapshots: Vec<_> = (0..36)
        .map(|index| 0x1357_0000 ^ (1 << (index % 8)) ^ if index % 2 == 0 { A0 | RESET } else { 0 })
        .collect();
    io.script(GPIOB, gpio_snapshots.clone());
    assert_eq!(run(&mut io, &mut delay), Ok(()));

    let trace = io.trace.borrow();
    assert_eq!(
        &trace[..12],
        [
            Event::Read(CR, 0xa5a5_ffff),
            Event::Write(CR, 0xa5a5_fff7),
            Event::Read(CLOCK, 0x1234_0000),
            Event::Write(CLOCK, 0x1234_0400),
            Event::Read(CR, 0x1357_9bff),
            Event::Write(CR, 0x1357_9bf7),
            Event::Read(CR, 0x2469_1ff7),
            Event::Write(CR, 0x2469_1f72),
            Event::Write(IE, 0),
            Event::Read(IE, 0),
            Event::Read(CR, 0xdead_0000),
            Event::Write(CR, 0xdead_0008),
        ]
    );
    let gpio_pairs: Vec<_> = trace
        .windows(2)
        .filter_map(|pair| match pair {
            [Event::Read(GPIOB, before), Event::Write(GPIOB, after)] => Some((*before, *after)),
            _ => None,
        })
        .collect();
    assert_eq!(gpio_pairs.len(), 36);
    for (index, ((before, after), snapshot)) in gpio_pairs.iter().zip(gpio_snapshots).enumerate() {
        assert_eq!(*before, snapshot);
        let mask = if index < 3 { RESET } else { A0 };
        assert_eq!(before & !mask, after & !mask);
    }
    let mode_pairs: Vec<_> = trace[12..]
        .windows(2)
        .filter_map(|pair| match pair {
            [Event::Read(CR, before), Event::Write(CR, after)] => Some((*before, *after)),
            _ => None,
        })
        .collect();
    assert_eq!(
        mode_pairs,
        [
            (0xaaaa_ffff, 0xaaaa_efff),
            (0xbbbb_0000, 0xbbbb_1000),
            (0xcccc_ffff, 0xcccc_efff),
            (0xdddd_0000, 0xdddd_1000),
        ]
    );
}

fn assert_restored(io: &MockRegisters) {
    assert_ne!(io.values[&CR] & MODE, 0);
    let trace = io.trace.borrow();
    match &trace[trace.len() - 2..] {
        [Event::Read(CR, before), Event::Write(CR, after)] => {
            assert_eq!(*after, before | MODE);
        }
        other => panic!("missing final mode restoration: {other:?}"),
    }
}

#[test]
fn fifo_timeout_at_every_command_stops_later_commands_and_restores_mode() {
    let all_bytes: Vec<_> = STARTUP_COMMANDS
        .into_iter()
        .chain((0..8).flat_map(|page| [0xb0 + page, 0x10, 0x04]))
        .collect();
    for failed_byte in 0..all_bytes.len() {
        let (mut io, mut delay) = mocks();
        io.script(FIFOST, std::iter::repeat_n(0, failed_byte));
        io.values.insert(FIFOST, 1 << 4);
        assert_eq!(run(&mut io, &mut delay), Err(LineError::FifoTimeout));
        assert_eq!(io.bytes(), all_bytes[..failed_byte]);
        assert_eq!(io.reads(FIFOST), failed_byte + 3);
        assert_restored(&io);
    }
}

#[test]
fn fifo_readiness_on_last_permitted_poll_succeeds_for_every_byte() {
    let (mut io, mut delay) = mocks();
    io.script(FIFOST, (0..41).flat_map(|_| [1 << 4, 1 << 4, 0]));
    assert_eq!(run(&mut io, &mut delay), Ok(()));
    assert_eq!(io.bytes().len(), 41);
    assert_eq!(io.reads(FIFOST), 41 * 3);
}

#[test]
fn timeout_at_every_drain_stops_later_pages_and_restores_mode() {
    // Initialization drain, then command-selection and empty-data drain per page.
    for failed_drain in 0..17 {
        let (mut io, mut delay) = mocks();
        io.script(IF, std::iter::repeat_n(0, failed_drain));
        io.values.insert(IF, 0x20);
        assert_eq!(run(&mut io, &mut delay), Err(LineError::DrainTimeout));
        assert_eq!(io.reads(IF), failed_drain + DRAIN_POLL_LIMIT as usize);
        let selected_pages = failed_drain.div_ceil(2);
        assert_eq!(io.bytes().len(), 17 + selected_pages * 3);
        let a0_high_writes = io
            .trace
            .borrow()
            .iter()
            .filter(|event| matches!(event, Event::Write(GPIOB, value) if value & A0 != 0))
            .count();
        // Three reset writes preserve initially high A0.
        assert_eq!(a0_high_writes, 3 + failed_drain / 2);
        assert_restored(&io);
    }
}

#[test]
fn every_drain_can_complete_on_its_last_permitted_read() {
    let (mut io, mut delay) = mocks();
    io.script(
        IF,
        (0..17).flat_map(|_| std::iter::repeat_n(0x20, DRAIN_POLL_LIMIT as usize - 1).chain([0])),
    );
    assert_eq!(run(&mut io, &mut delay), Ok(()));
    assert_eq!(io.reads(IF), 17 * DRAIN_POLL_LIMIT as usize);
    assert_eq!(io.bytes().len(), 41);
    assert_restored(&io);
}

#[test]
fn caller_supplied_delay_trait_object_runs_the_same_startup() {
    let (mut io, mut delay) = mocks();
    let delay: &mut dyn DelayMs = &mut delay;
    assert_eq!(
        startup_with_io(&mut io, delay, NonZeroU32::new(1).unwrap()),
        Ok(())
    );
    assert_eq!(io.bytes().len(), 41);
    assert_eq!(io.reads(FIFOST), 41);
}
