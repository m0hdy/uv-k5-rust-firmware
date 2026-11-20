use std::collections::VecDeque;
use uv_k5_rust_firmware::startup_battery_sampling::{
    AcquisitionError, CalibrationError, ReadRegister, RegisterIo, StartupBatteryError,
    StartupBatteryReadings, WriteRegister, sample_startup_battery,
};

#[derive(Debug, Eq, PartialEq)]
enum Event {
    Read(ReadRegister, u32),
    Write(WriteRegister, u32),
}

struct MockIo {
    reads: VecDeque<(ReadRegister, u32)>,
    events: Vec<Event>,
}

impl MockIo {
    fn new(reads: Vec<(ReadRegister, u32)>) -> Self {
        Self {
            reads: reads.into(),
            events: Vec::new(),
        }
    }
}

impl RegisterIo for MockIo {
    fn read(&mut self, register: ReadRegister) -> u32 {
        let (expected, value) = self.reads.pop_front().expect("unexpected read");
        assert_eq!(register, expected);
        self.events.push(Event::Read(register, value));
        value
    }

    fn write(&mut self, register: WriteRegister, value: u32) {
        self.events.push(Event::Write(register, value));
    }
}

fn successful_read(voltage: u32, current: u32) -> [(ReadRegister, u32); 4] {
    [
        (ReadRegister::Start, 0),
        (ReadRegister::Channel9Status, 1),
        (ReadRegister::Channel4Data, voltage),
        (ReadRegister::Channel9Data, current),
    ]
}

#[test]
fn four_ordered_acquisitions_calibrate_truncated_average_and_keep_last_current() {
    let mut io = MockIo::new(
        [
            successful_read(0x1001, 10),
            successful_read(1, 20),
            successful_read(1, 30),
            successful_read(2, 0x1028),
        ]
        .into_iter()
        .flatten()
        .collect(),
    );

    assert_eq!(
        sample_startup_battery(&mut io, 1, 3),
        Ok(StartupBatteryReadings {
            voltage_samples: [1, 1, 1, 2],
            current: 40,
            calibrated_voltage: 253,
        })
    );
    assert!(io.reads.is_empty());
    assert_eq!(
        io.events
            .iter()
            .filter(|event| matches!(event, Event::Write(WriteRegister::Start, 1)))
            .count(),
        4
    );
}

#[test]
fn acquisition_failure_reports_position_and_stops_without_partial_result() {
    for failed_index in 0..4 {
        let mut reads = Vec::new();
        for index in 0..failed_index {
            reads.extend(successful_read(index as u32 + 10, index as u32 + 20));
        }
        reads.extend([
            (ReadRegister::Start, 0),
            (ReadRegister::Channel9Status, 0),
            (ReadRegister::Channel9Status, 0),
        ]);
        let mut io = MockIo::new(reads);

        assert_eq!(
            sample_startup_battery(&mut io, 2, 760),
            Err(StartupBatteryError::Acquisition {
                sample_index: failed_index,
                error: AcquisitionError::PollBudgetExhausted,
            })
        );
        assert!(io.reads.is_empty());
        assert_eq!(
            io.events
                .iter()
                .filter(|event| matches!(event, Event::Write(WriteRegister::Start, 1)))
                .count(),
            failed_index + 1
        );
        assert_eq!(
            io.events
                .iter()
                .filter(|event| matches!(event, Event::Write(WriteRegister::InterruptFlags, _)))
                .count(),
            failed_index * 2
        );
    }
}

#[test]
fn zero_calibration_takes_precedence_and_performs_no_io() {
    let mut io = MockIo::new(Vec::new());

    assert_eq!(
        sample_startup_battery(&mut io, 0, 0),
        Err(StartupBatteryError::Calibration(
            CalibrationError::ZeroCalibration
        ))
    );
    assert!(io.events.is_empty());
}

#[test]
fn zero_poll_budget_fails_on_first_sample_without_io() {
    let mut io = MockIo::new(Vec::new());

    assert_eq!(
        sample_startup_battery(&mut io, 0, 760),
        Err(StartupBatteryError::Acquisition {
            sample_index: 0,
            error: AcquisitionError::PollBudgetExhausted,
        })
    );
    assert!(io.events.is_empty());
}
