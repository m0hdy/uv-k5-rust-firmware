#[allow(dead_code)]
#[path = "../src/backlight_startup_executor.rs"]
mod backlight_startup_executor;

use backlight_startup_executor::{Register, RegisterIo, apply_with_io};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Read(Register),
    Write(Register, u32),
}

struct MockRegisterIo {
    reads: Vec<(Register, u32)>,
    next_read: usize,
    events: Vec<Event>,
}

impl MockRegisterIo {
    fn new(reads: Vec<(Register, u32)>) -> Self {
        Self {
            reads,
            next_read: 0,
            events: Vec::new(),
        }
    }
}

impl RegisterIo for MockRegisterIo {
    fn read(&mut self, register: Register) -> u32 {
        let (expected_register, value) = self.reads[self.next_read];
        assert_eq!(register, expected_register);
        self.next_read += 1;
        self.events.push(Event::Read(register));
        value
    }

    fn write(&mut self, register: Register, value: u32) {
        self.events.push(Event::Write(register, value));
    }
}

#[test]
fn trace_uses_a_fresh_read_for_each_rmw_write() {
    let mut io = MockRegisterIo::new(vec![
        (Register::PwmPlus0ClockSource, 0xa123_4567),
        (Register::PortBSel0, 0x89ab_cdef),
        (Register::PortBSel0, 0x7654_3210),
    ]);

    apply_with_io(&mut io);

    assert_eq!(
        io.events,
        vec![
            Event::Read(Register::PwmPlus0ClockSource),
            Event::Write(Register::PwmPlus0ClockSource, 0xa12f_4567),
            Event::Write(Register::PwmPlus0Period, 1023),
            Event::Read(Register::PortBSel0),
            Event::Write(Register::PortBSel0, 0x80ab_cdef),
            Event::Read(Register::PortBSel0),
            Event::Write(Register::PortBSel0, 0x7754_3210),
            Event::Write(Register::PwmPlus0Generator, 0x0101_0000),
            Event::Write(Register::PwmPlus0Config, 5),
        ]
    );
    assert_eq!(io.next_read, io.reads.len());
}

#[test]
fn assignments_write_without_reading_the_assigned_registers() {
    let mut io = MockRegisterIo::new(vec![
        (Register::PwmPlus0ClockSource, 0),
        (Register::PortBSel0, 0),
        (Register::PortBSel0, 0),
    ]);

    apply_with_io(&mut io);

    assert_eq!(
        io.events
            .iter()
            .filter_map(|event| match event {
                Event::Read(register) => Some(*register),
                Event::Write(_, _) => None,
            })
            .collect::<Vec<_>>(),
        vec![
            Register::PwmPlus0ClockSource,
            Register::PortBSel0,
            Register::PortBSel0,
        ]
    );
    assert_eq!(
        io.events
            .iter()
            .filter(|event| matches!(
                event,
                Event::Write(
                    Register::PwmPlus0Period
                        | Register::PwmPlus0Generator
                        | Register::PwmPlus0Config,
                    _
                )
            ))
            .copied()
            .collect::<Vec<_>>(),
        vec![
            Event::Write(Register::PwmPlus0Period, 1023),
            Event::Write(Register::PwmPlus0Generator, 0x0101_0000),
            Event::Write(Register::PwmPlus0Config, 5),
        ]
    );
}
