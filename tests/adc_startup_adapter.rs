#[path = "../src/adc_startup_adapter.rs"]
mod adc_startup_adapter;

use adc_startup_adapter::{Interrupt, IrqAction, Register, RegisterIo, apply_with_io};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Read(Register, u32),
    Write(Register, u32),
    Irq(Interrupt, IrqAction),
}

struct FakeRegisterIo {
    values: [u32; 9],
    scripted_reads: [VecDeque<u32>; 9],
    events: Vec<Event>,
}

impl FakeRegisterIo {
    fn new(values: [u32; 9], scripted_reads: [VecDeque<u32>; 9]) -> Self {
        Self {
            values,
            scripted_reads,
            events: Vec::new(),
        }
    }

    fn index(register: Register) -> usize {
        match register {
            Register::DeviceClockGate => 0,
            Register::ClockSelector => 1,
            Register::Configuration => 2,
            Register::ExternalTriggerSelector => 3,
            Register::CalibrationOffset => 4,
            Register::CalibrationKd => 5,
            Register::InterruptFlags => 6,
            Register::InterruptEnable => 7,
            Register::Start => 8,
        }
    }
}

impl RegisterIo for FakeRegisterIo {
    fn read(&mut self, register: Register) -> u32 {
        let index = Self::index(register);
        let value = self.scripted_reads[index]
            .pop_front()
            .unwrap_or(self.values[index]);
        self.events.push(Event::Read(register, value));
        value
    }

    fn write(&mut self, register: Register, value: u32) {
        self.events.push(Event::Write(register, value));
        self.values[Self::index(register)] = value;
    }

    fn set_irq(&mut self, interrupt: Interrupt, action: IrqAction) {
        self.events.push(Event::Irq(interrupt, action));
    }
}

fn empty_reads() -> [VecDeque<u32>; 9] {
    core::array::from_fn(|_| VecDeque::new())
}

#[test]
fn address_free_register_interface_records_the_full_order_and_fresh_reads() {
    let mut reads = empty_reads();
    reads[0] = VecDeque::from([0xa5a5_51aa]);
    reads[1] = VecDeque::from([0xa5a5_5aaa]);
    reads[2] = VecDeque::from([0xafff_ffff, 0x1357_9bff, 0xd6ad_0000]);
    reads[4] = VecDeque::from([0xa5a4_5aaa]);
    reads[5] = VecDeque::from([0x1356_9bdf]);
    reads[7] = VecDeque::from([0xdead_beef, 0]);
    reads[8] = VecDeque::from([0xa5a5_5aaf, 0x2468_1ff3]);

    let mut io = FakeRegisterIo::new([0; 9], reads);
    apply_with_io(&mut io, 0xa55a);

    assert_eq!(
        io.events,
        vec![
            Event::Read(Register::DeviceClockGate, 0xa5a5_51aa),
            Event::Write(Register::DeviceClockGate, 0xa7a5_51aa),
            Event::Read(Register::Configuration, 0xafff_ffff),
            Event::Write(Register::Configuration, 0xa7ff_ffff),
            Event::Read(Register::ClockSelector, 0xa5a5_5aaa),
            Event::Write(Register::ClockSelector, 0xa5a5_56aa),
            Event::Read(Register::Configuration, 0x1357_9bff),
            Event::Write(Register::Configuration, 0x07c3_8210),
            Event::Write(Register::ExternalTriggerSelector, 0xa55a),
            Event::Read(Register::CalibrationOffset, 0xa5a4_5aaa),
            Event::Write(Register::CalibrationOffset, 0xa5a5_5aaa),
            Event::Read(Register::CalibrationKd, 0x1356_9bdf),
            Event::Write(Register::CalibrationKd, 0x1357_9bdf),
            Event::Write(Register::InterruptFlags, u32::MAX),
            Event::Read(Register::InterruptEnable, 0xdead_beef),
            Event::Write(Register::InterruptEnable, 0xdeac_0000),
            Event::Read(Register::InterruptEnable, 0),
            Event::Irq(Interrupt::Saradc, IrqAction::Disable),
            Event::Read(Register::Configuration, 0xd6ad_0000),
            Event::Write(Register::Configuration, 0xdead_0000),
            Event::Read(Register::Start, 0xa5a5_5aaf),
            Event::Write(Register::Start, 0xa5a5_5aab),
            Event::Read(Register::Start, 0x2468_1ff3),
            Event::Write(Register::Start, 0x2468_1ff7),
        ]
    );
}

#[test]
fn nonzero_interrupt_enable_readback_enables_saradc_irq() {
    let mut reads = empty_reads();
    reads[7] = VecDeque::from([0, 0x8000_0000]);

    let mut io = FakeRegisterIo::new([0; 9], reads);
    apply_with_io(&mut io, 0x1234);

    assert_eq!(
        io.events
            .iter()
            .filter(|event| matches!(event, Event::Irq(_, _)))
            .copied()
            .collect::<Vec<_>>(),
        [Event::Irq(Interrupt::Saradc, IrqAction::Enable)]
    );
    assert_eq!(
        io.events
            .iter()
            .filter(|event| matches!(event, Event::Read(Register::InterruptEnable, _)))
            .count(),
        2
    );
}
