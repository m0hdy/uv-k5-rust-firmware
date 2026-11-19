use std::collections::{BTreeMap, VecDeque};
use std::num::NonZeroU32;

use uv_k5_rust_firmware::boot_coordinator::{
    Acknowledgement, BatteryReadRegister, BatteryRegisterIo, BatteryWriteRegister, BoardCall,
    BoardDefines, BoardStartupError, BootConfig, BootError, I2cTransport, SettingsBootBlock,
    SettingsBootReadError, StartupIo, coordinate,
};
use uv_k5_rust_firmware::settings_boot_composition::{Features as SettingsFeatures, ReadError};
use uv_k5_rust_firmware::st7565_line_transport::LineError;
use uv_k5_rust_firmware::st7565_startup_executor::target_spi0::{
    DelayMs, RegisterIo as DisplayRegisterIo, startup_with_io,
};

const CORE_DATA: [u8; 8] = [73, 8, 10, 1, 1, 1, 9, 4];
const DISPLAY_DATA: [u8; 8] = [0x24, 3, 1, 4, 2, 5, 1, 1];
const SPI_CR: usize = 0x400b_8000;
const SPI_WDR: usize = 0x400b_8004;
const SPI_IE: usize = 0x400b_8010;
const SPI_IF: usize = 0x400b_8014;
const SPI_FIFOST: usize = 0x400b_8018;
const GPIOB_DATA: usize = 0x4006_0800;
const SYSCON_CLOCK: usize = 0x4000_0008;
const ST7565_COMMANDS: [u8; 17] = [
    0xe2, 0xa2, 0xc0, 0xa1, 0xa6, 0xa4, 0x24, 0x81, 0x1f, 0x2b, 0x2e, 0x2f, 0x2f, 0x2f, 0x2f, 0x40,
    0xaf,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    DeviceClockGates,
    Systick(u32),
    Portcon(bool),
    Gpio(bool),
    BacklightInit,
    AdcInit(u16),
    St7565Init(NonZeroU32),
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
enum DisplayEvent {
    Read(usize, u32),
    Write(usize, u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EepromError {
    Transfer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Failure {
    None,
    Systick,
    Display,
    Eeprom,
    Battery,
}

struct MockIo {
    events: Vec<Event>,
    failure: Failure,
    settings_responses: [[u8; 8]; 2],
    settings_reads: usize,
    battery_reads: VecDeque<(BatteryReadRegister, u32)>,
    display_registers: BTreeMap<usize, u32>,
    display_trace: Vec<DisplayEvent>,
}

#[derive(Default)]
struct DelayProbe(Vec<u16>);

impl DelayMs for DelayProbe {
    fn delay_ms(&mut self, milliseconds: u16) {
        self.0.push(milliseconds);
    }
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
        let display_registers = [
            (SPI_CR, 0xa5a5_ffff),
            (SPI_WDR, 0),
            (SPI_IE, u32::MAX),
            (SPI_IF, 0),
            (SPI_FIFOST, u32::from(failure == Failure::Display) << 4),
            (GPIOB_DATA, 0x5a5a_f7ff),
            (SYSCON_CLOCK, 0x1234_0000),
        ]
        .into();
        Self {
            events: Vec::new(),
            failure,
            settings_responses: [CORE_DATA, DISPLAY_DATA],
            settings_reads: 0,
            battery_reads,
            display_registers,
            display_trace: Vec::new(),
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
            Err(BoardStartupError::SysTick)
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

    fn backlight_init_hardware(&mut self) {
        self.events.push(Event::BacklightInit);
    }

    fn adc_init(&mut self, exttrig_sel: u16) {
        self.events.push(Event::AdcInit(exttrig_sel));
    }

    fn st7565_init(
        &mut self,
        delay: &mut dyn DelayMs,
        fifo_poll_limit: NonZeroU32,
    ) -> Result<(), LineError> {
        self.events.push(Event::St7565Init(fifo_poll_limit));
        startup_with_io(self, delay, fifo_poll_limit)
    }

    fn placeholder(&mut self, call: BoardCall) {
        self.events.push(Event::BoardPlaceholder(call));
    }
}

impl DisplayRegisterIo for MockIo {
    fn read(&mut self, address: usize) -> u32 {
        let value = *self
            .display_registers
            .get(&address)
            .expect("known ST7565 register");
        self.display_trace.push(DisplayEvent::Read(address, value));
        value
    }

    fn write(&mut self, address: usize, value: u32) {
        *self
            .display_registers
            .get_mut(&address)
            .expect("known ST7565 register") = value;
        self.display_trace.push(DisplayEvent::Write(address, value));
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
        exttrig_sel: 0x5aa5,
        st7565_fifo_poll_limit: NonZeroU32::new(0x1234).unwrap(),
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
    let mut delay = DelayProbe::default();
    let state = coordinate(&mut io, &mut delay, config()).unwrap();

    assert_eq!(state.settings.core.call_channel, 73);
    assert_eq!(state.settings.display.channel_display_mode, 3);
    assert_eq!(state.battery.voltage_samples, [1, 2, 1, 2]);
    assert_eq!(state.battery.current, 13);

    let last_board = io
        .events
        .iter()
        .rposition(|event| matches!(event, Event::St7565Init(_) | Event::BoardPlaceholder(_)))
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
            Event::BacklightInit,
            Event::AdcInit(0x5aa5),
            Event::St7565Init(NonZeroU32::new(0x1234).unwrap()),
        ]
    );
    assert_eq!(
        io.display_trace
            .iter()
            .filter_map(|event| match event {
                DisplayEvent::Write(SPI_WDR, byte) => Some(*byte as u8),
                _ => None,
            })
            .collect::<Vec<_>>(),
        ST7565_COMMANDS
            .into_iter()
            .chain((0..8).flat_map(|page| [0xb0 + page, 0x10, 0x04]))
            .collect::<Vec<_>>()
    );
    assert_eq!(delay.0, [1, 20, 120, 120, 1, 1, 40]);
    assert_eq!(
        io.events
            .iter()
            .filter(|event| matches!(event, Event::St7565Init(_)))
            .count(),
        1
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
    let mut delay = DelayProbe::default();

    assert_eq!(
        coordinate(&mut io, &mut delay, config()),
        Err(BootError::Board(BoardStartupError::SysTick))
    );
    assert_eq!(
        io.events,
        [Event::DeviceClockGates, Event::Systick(480_000)]
    );
}

#[test]
fn display_failure_preserves_cause_and_stops_before_settings_and_battery() {
    let mut io = MockIo::new(Failure::Display);
    let mut delay = DelayProbe::default();

    assert_eq!(
        coordinate(&mut io, &mut delay, config()),
        Err(BootError::Board(BoardStartupError::St7565(
            LineError::FifoTimeout
        )))
    );
    assert!(matches!(io.events.last(), Some(Event::St7565Init(_))));
    assert!(!io.events.iter().any(|event| {
        matches!(
            event,
            Event::EepromStart | Event::BatteryRead(_) | Event::BatteryWrite(_, _)
        )
    }));
    assert_eq!(delay.0, [1, 20, 120]);
}

#[test]
fn eeprom_failure_stops_before_battery_and_returns_no_app_state() {
    let mut io = MockIo::new(Failure::Eeprom);
    let mut delay = DelayProbe::default();

    assert_eq!(
        coordinate(&mut io, &mut delay, config()),
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
    let mut delay = DelayProbe::default();

    assert!(matches!(
        coordinate(&mut io, &mut delay, config()),
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
