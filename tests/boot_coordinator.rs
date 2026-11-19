use std::collections::VecDeque;

use uv_k5_rust_firmware::boot_coordinator::{
    Acknowledgement, BatteryReadRegister, BatteryRegisterIo, BatteryWriteRegister, BoardCall,
    BoardDefines, BoardStartupError, BootConfig, BootError, I2cTransport, SettingsBootBlock,
    SettingsBootReadError, StartupIo, coordinate,
};
use uv_k5_rust_firmware::settings_boot_composition::{Features as SettingsFeatures, ReadError};

const CORE_DATA: [u8; 8] = [73, 8, 10, 1, 1, 1, 9, 4];
const DISPLAY_DATA: [u8; 8] = [0x24, 3, 1, 4, 2, 5, 1, 1];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    DeviceClockGates,
    Systick(u32),
    Portcon(bool),
    Gpio(bool),
    BoardPlaceholder(BoardCall),
    EepromStart,
    EepromWrite(u8),
    EepromRepeatedStart,
    EepromRead(usize),
    EepromStop,
    BatteryRead(BatteryReadRegister),
    BatteryWrite(BatteryWriteRegister, u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EepromError {
    Transfer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Failure {
    None,
    Systick,
    Eeprom,
    Battery,
}

struct MockIo {
    events: Vec<Event>,
    failure: Failure,
    settings_responses: [[u8; 8]; 2],
    settings_reads: usize,
    battery_reads: VecDeque<(BatteryReadRegister, u32)>,
}

impl MockIo {
    fn new(failure: Failure) -> Self {
        let battery_reads = (0..4)
            .flat_map(|index| {
                [
                    (BatteryReadRegister::Start, 0),
                    (
                        BatteryReadRegister::Channel9Status,
                        u32::from(failure != Failure::Battery),
                    ),
                    (BatteryReadRegister::Channel4Data, index % 2 + 1),
                    (BatteryReadRegister::Channel9Data, index + 10),
                ]
            })
            .collect();
        Self {
            events: Vec::new(),
            failure,
            settings_responses: [CORE_DATA, DISPLAY_DATA],
            settings_reads: 0,
            battery_reads,
        }
    }

    fn record_eeprom(&mut self, event: Event) -> Result<(), EepromError> {
        self.events.push(event);
        if self.failure == Failure::Eeprom && matches!(event, Event::EepromWrite(_)) {
            Err(EepromError::Transfer)
        } else {
            Ok(())
        }
    }
}

impl StartupIo for MockIo {
    fn device_clock_gates(&mut self) {
        self.events.push(Event::DeviceClockGates);
    }

    fn systick(&mut self, period_ticks: u32) -> Result<(), BoardStartupError> {
        self.events.push(Event::Systick(period_ticks));
        if self.failure == Failure::Systick {
            Err(BoardStartupError)
        } else {
            Ok(())
        }
    }

    fn portcon(&mut self, enable_swd: bool) {
        self.events.push(Event::Portcon(enable_swd));
    }

    fn gpio(&mut self, enable_fmradio: bool) {
        self.events.push(Event::Gpio(enable_fmradio));
    }

    fn placeholder(&mut self, call: BoardCall) {
        self.events.push(Event::BoardPlaceholder(call));
    }
}

impl I2cTransport for MockIo {
    type Error = EepromError;

    fn start(&mut self) -> Result<(), Self::Error> {
        self.record_eeprom(Event::EepromStart)
    }

    fn repeated_start(&mut self) -> Result<(), Self::Error> {
        self.record_eeprom(Event::EepromRepeatedStart)
    }

    fn write_byte(&mut self, byte: u8) -> Result<Acknowledgement, Self::Error> {
        self.record_eeprom(Event::EepromWrite(byte))?;
        Ok(Acknowledgement::Ack)
    }

    fn read_buffer(&mut self, output: &mut [u8]) -> Result<(), Self::Error> {
        self.events.push(Event::EepromRead(output.len()));
        output.copy_from_slice(&self.settings_responses[self.settings_reads]);
        self.settings_reads += 1;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        self.record_eeprom(Event::EepromStop)
    }
}

impl BatteryRegisterIo for MockIo {
    fn read(&mut self, register: BatteryReadRegister) -> u32 {
        let (expected, value) = self.battery_reads.pop_front().expect("unexpected ADC read");
        assert_eq!(register, expected);
        self.events.push(Event::BatteryRead(register));
        value
    }

    fn write(&mut self, register: BatteryWriteRegister, value: u32) {
        self.events.push(Event::BatteryWrite(register, value));
    }
}

fn config() -> BootConfig {
    BootConfig {
        board: BoardDefines {
            enable_fmradio: false,
            enable_uart: false,
            enabled_aircopy: false,
        },
        enable_swd: false,
        settings: SettingsFeatures {
            noaa_enabled: true,
            vox_enabled: true,
            blmin_tmp_off_enabled: true,
        },
        battery_poll_budget: 1,
        battery_calibration: 3,
    }
}

#[test]
fn success_runs_board_then_fallible_settings_then_bounded_battery_sampling() {
    let mut io = MockIo::new(Failure::None);
    let state = coordinate(&mut io, config()).unwrap();

    assert_eq!(state.settings.core.call_channel, 73);
    assert_eq!(state.settings.display.channel_display_mode, 3);
    assert_eq!(state.battery.voltage_samples, [1, 2, 1, 2]);
    assert_eq!(state.battery.current, 13);

    let last_board = io
        .events
        .iter()
        .rposition(|event| matches!(event, Event::BoardPlaceholder(_)))
        .unwrap();
    let first_eeprom = io
        .events
        .iter()
        .position(|event| *event == Event::EepromStart)
        .unwrap();
    let last_eeprom = io
        .events
        .iter()
        .rposition(|event| *event == Event::EepromStop)
        .unwrap();
    let first_battery = io
        .events
        .iter()
        .position(|event| matches!(event, Event::BatteryRead(_)))
        .unwrap();
    assert!(last_board < first_eeprom);
    assert!(last_eeprom < first_battery);
    assert_eq!(
        io.events[..7],
        [
            Event::DeviceClockGates,
            Event::Systick(480_000),
            Event::Portcon(false),
            Event::Gpio(false),
            Event::BoardPlaceholder(BoardCall::BacklightInitHardware),
            Event::BoardPlaceholder(BoardCall::AdcInit),
            Event::BoardPlaceholder(BoardCall::St7565Init),
        ]
    );
    assert_eq!(
        io.events
            .iter()
            .filter(|event| matches!(event, Event::EepromRead(8)))
            .count(),
        2
    );
}

#[test]
fn board_failure_stops_before_eeprom_and_battery_and_returns_no_app_state() {
    let mut io = MockIo::new(Failure::Systick);

    assert_eq!(
        coordinate(&mut io, config()),
        Err(BootError::Board(BoardStartupError))
    );
    assert_eq!(
        io.events,
        [Event::DeviceClockGates, Event::Systick(480_000)]
    );
}

#[test]
fn eeprom_failure_stops_before_battery_and_returns_no_app_state() {
    let mut io = MockIo::new(Failure::Eeprom);

    assert_eq!(
        coordinate(&mut io, config()),
        Err(BootError::Settings(SettingsBootReadError {
            block: SettingsBootBlock::Core,
            error: ReadError::Transport(EepromError::Transfer),
        }))
    );
    assert!(
        !io.events
            .iter()
            .any(|event| matches!(event, Event::BatteryRead(_)))
    );
}

#[test]
fn battery_failure_returns_no_app_state_after_complete_settings_reads() {
    let mut io = MockIo::new(Failure::Battery);

    assert!(matches!(
        coordinate(&mut io, config()),
        Err(BootError::Battery(_))
    ));
    assert_eq!(
        io.events
            .iter()
            .filter(|event| matches!(event, Event::EepromRead(8)))
            .count(),
        2
    );
    assert_eq!(
        io.events
            .iter()
            .filter(|event| matches!(event, Event::BatteryWrite(BatteryWriteRegister::Start, 1)))
            .count(),
        1
    );
}
