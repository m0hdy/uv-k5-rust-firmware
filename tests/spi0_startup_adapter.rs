#[path = "../src/spi0_startup_adapter.rs"]
mod spi0_startup_adapter;

use spi0_startup_adapter::{RegisterAddress, RegisterIo, apply_with_io};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Read(RegisterAddress, u32),
    Write(RegisterAddress, u32),
}

struct FakeRegisterIo {
    values: [u32; 3],
    scripted_reads: [VecDeque<u32>; 3],
    events: Vec<Event>,
}

impl FakeRegisterIo {
    fn new(values: [u32; 3], scripted_reads: [VecDeque<u32>; 3]) -> Self {
        Self {
            values,
            scripted_reads,
            events: Vec::new(),
        }
    }

    fn index(register: RegisterAddress) -> usize {
        match register {
            RegisterAddress::Spi0Control => 0,
            RegisterAddress::Spi0InterruptEnable => 1,
            RegisterAddress::DeviceClockGate => 2,
        }
    }
}

impl RegisterIo for FakeRegisterIo {
    fn read(&mut self, register: RegisterAddress) -> u32 {
        let index = Self::index(register);
        let value = self.scripted_reads[index]
            .pop_front()
            .unwrap_or(self.values[index]);
        self.events.push(Event::Read(register, value));
        value
    }

    fn write(&mut self, register: RegisterAddress, value: u32) {
        self.events.push(Event::Write(register, value));
        self.values[Self::index(register)] = value;
    }
}

#[test]
fn address_map_contains_only_the_three_spi0_startup_registers() {
    assert_eq!(
        [
            RegisterAddress::Spi0Control.address(),
            RegisterAddress::Spi0InterruptEnable.address(),
            RegisterAddress::DeviceClockGate.address(),
        ],
        [0x400b_8000, 0x400b_8010, 0x4000_0008]
    );
}

#[test]
fn initialization_performs_literal_ordered_accesses_using_fresh_reads() {
    let mut io = FakeRegisterIo::new(
        [0, u32::MAX, 0xa5a5_51aa],
        [
            VecDeque::from([0xa5a5_ffff, 0x1357_9bff, 0x2468_1ff7, 0xdead_0000]),
            VecDeque::new(),
            VecDeque::new(),
        ],
    );

    apply_with_io(&mut io);

    assert_eq!(
        io.events,
        vec![
            Event::Read(RegisterAddress::Spi0Control, 0xa5a5_ffff),
            Event::Write(RegisterAddress::Spi0Control, 0xa5a5_fff7),
            Event::Read(RegisterAddress::DeviceClockGate, 0xa5a5_51aa),
            Event::Write(RegisterAddress::DeviceClockGate, 0xa5a5_55aa),
            Event::Read(RegisterAddress::Spi0Control, 0x1357_9bff),
            Event::Write(RegisterAddress::Spi0Control, 0x1357_9bf7),
            Event::Read(RegisterAddress::Spi0Control, 0x2468_1ff7),
            Event::Write(RegisterAddress::Spi0Control, 0x2468_1f72),
            Event::Write(RegisterAddress::Spi0InterruptEnable, 0),
            Event::Read(RegisterAddress::Spi0InterruptEnable, 0),
            Event::Read(RegisterAddress::Spi0Control, 0xdead_0000),
            Event::Write(RegisterAddress::Spi0Control, 0xdead_0008),
        ]
    );
}

#[test]
fn configuration_preserves_unrelated_bits_including_tf_clr() {
    let mut io = FakeRegisterIo::new(
        [0, 0, 0],
        [
            VecDeque::from([0, 0, 0x2469_1ff7, 0]),
            VecDeque::new(),
            VecDeque::new(),
        ],
    );

    apply_with_io(&mut io);

    let configured_control = io.events[7];
    assert_eq!(
        configured_control,
        Event::Write(RegisterAddress::Spi0Control, 0x2469_1f72)
    );
    if let Event::Write(_, value) = configured_control {
        assert_eq!(value & (1 << 16), 1 << 16);
        assert_eq!(value & 0x2469_0000, 0x2469_0000);
        assert_eq!(value & 0x7, 2);
        assert_eq!(value & (1 << 4), 1 << 4);
        assert_eq!(value & (1 << 5), 1 << 5);
        assert_eq!(value & (1 << 6), 1 << 6);
        assert_eq!(value & (1 << 7), 0);
        assert_eq!(value & (1 << 15), 0);
    }
}

#[test]
fn zero_interrupt_enable_write_is_followed_by_a_read_without_nvic_access() {
    let mut io = FakeRegisterIo::new(
        [0, u32::MAX, 1 << 10],
        [VecDeque::new(), VecDeque::new(), VecDeque::new()],
    );

    apply_with_io(&mut io);

    let interrupt_events: Vec<_> = io
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::Read(RegisterAddress::Spi0InterruptEnable, _)
                    | Event::Write(RegisterAddress::Spi0InterruptEnable, _)
            )
        })
        .copied()
        .collect();
    assert_eq!(
        interrupt_events,
        vec![
            Event::Write(RegisterAddress::Spi0InterruptEnable, 0),
            Event::Read(RegisterAddress::Spi0InterruptEnable, 0),
        ]
    );
    assert_eq!(
        io.events
            .iter()
            .filter(|event| matches!(event, Event::Write(_, _)))
            .count(),
        6
    );
}

#[test]
fn unexpected_nonzero_interrupt_readback_is_ignored_without_extra_accesses() {
    let mut io = FakeRegisterIo::new(
        [0, 0, 0],
        [VecDeque::new(), VecDeque::from([0x15]), VecDeque::new()],
    );

    apply_with_io(&mut io);

    assert_eq!(
        io.events
            .iter()
            .filter(|event| matches!(
                event,
                Event::Read(RegisterAddress::Spi0InterruptEnable, 0x15)
            ))
            .count(),
        1
    );
    assert_eq!(io.events.len(), 12);
}
