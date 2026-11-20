use std::collections::VecDeque;
use uv_k5_rust_firmware::adc_battery_acquisition::{
    AcquisitionError, BatterySamples, ReadRegister, RegisterIo, WriteRegister,
    acquire_battery_samples,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Read(ReadRegister, u32),
    Write(WriteRegister, u32),
}

struct FakeRegisterIo {
    scripted_reads: [VecDeque<u32>; 4],
    events: Vec<Event>,
}

impl FakeRegisterIo {
    fn new(scripted_reads: [VecDeque<u32>; 4]) -> Self {
        Self {
            scripted_reads,
            events: Vec::new(),
        }
    }

    fn index(register: ReadRegister) -> usize {
        match register {
            ReadRegister::Start => 0,
            ReadRegister::Channel9Status => 1,
            ReadRegister::Channel4Data => 2,
            ReadRegister::Channel9Data => 3,
        }
    }
}

impl RegisterIo for FakeRegisterIo {
    fn read(&mut self, register: ReadRegister) -> u32 {
        let value = self.scripted_reads[Self::index(register)]
            .pop_front()
            .expect("unexpected read");
        self.events.push(Event::Read(register, value));
        value
    }

    fn write(&mut self, register: WriteRegister, value: u32) {
        self.events.push(Event::Write(register, value));
    }
}

fn empty_reads() -> [VecDeque<u32>; 4] {
    core::array::from_fn(|_| VecDeque::new())
}

#[test]
fn zero_poll_budget_performs_no_io() {
    let mut io = FakeRegisterIo::new(empty_reads());

    assert_eq!(
        acquire_battery_samples(&mut io, 0),
        Err(AcquisitionError::PollBudgetExhausted)
    );
    assert!(io.events.is_empty());
}

#[test]
fn first_ready_poll_acknowledges_channels_before_data_and_returns_voltage_then_current() {
    let mut reads = empty_reads();
    reads[0] = VecDeque::from([0xa5a5_5aae]);
    reads[1] = VecDeque::from([0xdead_beef]);
    reads[2] = VecDeque::from([0xdead_beef]);
    reads[3] = VecDeque::from([0x1234_5678]);
    let mut io = FakeRegisterIo::new(reads);

    assert_eq!(
        acquire_battery_samples(&mut io, 3),
        Ok(BatterySamples {
            voltage: 0x0eef,
            current: 0x0678,
        })
    );
    assert_eq!(
        io.events,
        vec![
            Event::Read(ReadRegister::Start, 0xa5a5_5aae),
            Event::Write(WriteRegister::Start, 0xa5a5_5aaf),
            Event::Read(ReadRegister::Channel9Status, 0xdead_beef),
            Event::Write(WriteRegister::InterruptFlags, 1 << 4),
            Event::Read(ReadRegister::Channel4Data, 0xdead_beef),
            Event::Write(WriteRegister::InterruptFlags, 1 << 9),
            Event::Read(ReadRegister::Channel9Data, 0x1234_5678),
        ]
    );
}

#[test]
fn readiness_on_final_allowed_poll_is_accepted() {
    let mut reads = empty_reads();
    reads[0] = VecDeque::from([0]);
    reads[1] = VecDeque::from([0, 0, 1]);
    reads[2] = VecDeque::from([0xabc]);
    reads[3] = VecDeque::from([0xdef]);
    let mut io = FakeRegisterIo::new(reads);

    assert_eq!(
        acquire_battery_samples(&mut io, 3),
        Ok(BatterySamples {
            voltage: 0xabc,
            current: 0xdef,
        })
    );
    assert_eq!(
        io.events
            .iter()
            .filter(|event| matches!(event, Event::Read(ReadRegister::Channel9Status, _)))
            .count(),
        3
    );
}

#[test]
fn exhausted_poll_budget_does_not_acknowledge_channels_or_read_data() {
    let mut reads = empty_reads();
    reads[0] = VecDeque::from([0x8000_0000]);
    reads[1] = VecDeque::from([0, 0]);
    let mut io = FakeRegisterIo::new(reads);

    assert_eq!(
        acquire_battery_samples(&mut io, 2),
        Err(AcquisitionError::PollBudgetExhausted)
    );
    assert_eq!(
        io.events,
        vec![
            Event::Read(ReadRegister::Start, 0x8000_0000),
            Event::Write(WriteRegister::Start, 0x8000_0001),
            Event::Read(ReadRegister::Channel9Status, 0),
            Event::Read(ReadRegister::Channel9Status, 0),
        ]
    );
}
