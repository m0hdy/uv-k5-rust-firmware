use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::num::NonZeroU32;
use uv_k5_rust_firmware::bk1080_startup_adapter::{self, ShutdownError, shutdown_with_io};
use uv_k5_rust_firmware::i2c_bitbang_flow::target_gpio::Delay1us;

#[path = "../src/crc_startup_adapter.rs"]
mod crc_startup_adapter;

use uv_k5_rust_firmware::boot_coordinator::{
    Acknowledgement, BatteryReadRegister, BatteryRegisterIo, BatteryWriteRegister, BoardDefines,
    BoardStartupError, BootConfig, BootError, BootIo, FirmwareStartupError, I2cTransport,
    SettingsBootBlock, SettingsBootReadError, StartupIo, coordinate, coordinate_with_welcome,
};
use uv_k5_rust_firmware::settings_boot_composition::{Features as SettingsFeatures, ReadError};
use uv_k5_rust_firmware::st7565_line_transport::LineError;
use uv_k5_rust_firmware::st7565_startup_executor::target_spi0::{
    DelayMs, RegisterIo as DisplayRegisterIo, startup_with_io,
};
use uv_k5_rust_firmware::startup_version_display::{
    StartupDisplayMode, StartupVersionDisplayPlan, startup_version_display_plan,
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
    CrcWrite(usize, u32),
    FmStart,
    FmWrite(u8),
    FmStop,
    FmRead(usize),
    FmGpioWrite(usize, u32),
    EepromStart,
    EepromWrite(u8),
    EepromRepeatedStart,
    EepromRead(usize),
    EepromStop,
    I2cHandoff,
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
    FmStart,
    FmNack(usize),
    FmWrite(usize),
    FmStop,
    FmCleanupStop(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FmError {
    Start,
    Write(usize),
    Stop,
}

struct FmBus<'a> {
    trace: &'a RefCell<Vec<Event>>,
    failure: Failure,
    writes: usize,
}
impl I2cTransport for FmBus<'_> {
    type Error = FmError;
    fn start(&mut self) -> Result<(), FmError> {
        self.trace.borrow_mut().push(Event::FmStart);
        if self.failure == Failure::FmStart {
            Err(FmError::Start)
        } else {
            Ok(())
        }
    }
    fn repeated_start(&mut self) -> Result<(), FmError> {
        unreachable!()
    }
    fn write_byte(&mut self, byte: u8) -> Result<Acknowledgement, FmError> {
        self.trace.borrow_mut().push(Event::FmWrite(byte));
        let index = self.writes;
        self.writes += 1;
        match self.failure {
            Failure::FmNack(i) if i == index => Ok(Acknowledgement::Nack),
            Failure::FmWrite(i) | Failure::FmCleanupStop(i) if i == index => {
                Err(FmError::Write(index))
            }
            _ => Ok(Acknowledgement::Ack),
        }
    }
    fn read_buffer(&mut self, _: &mut [u8]) -> Result<(), FmError> {
        unreachable!()
    }
    fn stop(&mut self) -> Result<(), FmError> {
        self.trace.borrow_mut().push(Event::FmStop);
        if matches!(self.failure, Failure::FmStop | Failure::FmCleanupStop(_)) {
            Err(FmError::Stop)
        } else {
            Ok(())
        }
    }
}

struct FmGpio<'a>(&'a RefCell<Vec<Event>>);
impl bk1080_startup_adapter::WordIo for FmGpio<'_> {
    fn read_word(&mut self, address: usize) -> u32 {
        self.0.borrow_mut().push(Event::FmRead(address));
        5
    }
    fn write_word(&mut self, address: usize, value: u32) {
        self.0.borrow_mut().push(Event::FmGpioWrite(address, value));
    }
}

struct MockIo {
    events: Vec<Event>,
    failure: Failure,
    settings_responses: [[u8; 8]; 3],
    settings_reads: usize,
    battery_reads: VecDeque<(BatteryReadRegister, u32)>,
    display_registers: BTreeMap<usize, u32>,
    display_trace: Vec<DisplayEvent>,
}

#[derive(Default)]
struct DelayProbe(Vec<u16>);

impl Delay1us for DelayProbe {
    fn delay_1us(&mut self) {
        self.0.push(1);
    }
}

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
            settings_responses: [CORE_DATA, DISPLAY_DATA, [0, 0, 0, 0, 0, 0, 0, 2]],
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
    type FmError = FmError;
    fn device_clock_gates(&mut self) {
        self.events.push(Event::DeviceClockGates);
    }

    fn systick(&mut self, period_ticks: u32) -> Result<(), BoardStartupError<Self::FmError>> {
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

    fn bk1080_shutdown(
        &mut self,
        delay: &mut dyn Delay1us,
    ) -> Result<(), ShutdownError<Self::FmError>> {
        delay.delay_1us();
        let trace = RefCell::new(Vec::new());
        let result = shutdown_with_io(
            &mut FmBus {
                trace: &trace,
                failure: self.failure,
                writes: 0,
            },
            &mut FmGpio(&trace),
        );
        self.events.extend(trace.into_inner());
        result
    }

    fn crc_init(&mut self) {
        crc_startup_adapter::apply_with_io(self);
    }
}

impl crc_startup_adapter::WordIo for MockIo {
    fn write_word(&mut self, address: usize, value: u32) {
        self.events.push(Event::CrcWrite(address, value));
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

impl BootIo for MockIo {
    fn i2c_handoff(&mut self) {
        self.events.push(Event::I2cHandoff);
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
    let state = coordinate(&mut io, &mut delay, &mut DelayProbe::default(), config()).unwrap();

    assert_eq!(state.settings.core.call_channel, 73);
    assert_eq!(state.settings.display.channel_display_mode, 3);
    assert_eq!(state.battery.voltage_samples, [1, 2, 1, 2]);
    assert_eq!(state.battery.current, 13);

    let last_board = io
        .events
        .iter()
        .rposition(|event| matches!(event, Event::St7565Init(_) | Event::FmGpioWrite(_, _)))
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
    let handoff = io
        .events
        .iter()
        .position(|event| *event == Event::I2cHandoff)
        .unwrap();
    assert!(last_board < first_eeprom);
    assert!(last_eeprom < first_battery);
    assert!(last_eeprom < handoff && handoff < first_battery);
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
        3
    );
}

#[test]
fn composed_startup_uses_acquired_mode_and_version_on_the_initialized_display() {
    for (raw, mode) in [
        (0, StartupDisplayMode::FullScreen),
        (1, StartupDisplayMode::Custom),
        (2, StartupDisplayMode::Voltage),
        (3, StartupDisplayMode::None),
    ] {
        let mut io = MockIo::new(Failure::None);
        io.settings_responses[2][7] = raw;
        let mut pages = [[0; 128]; 8];
        let startup = coordinate_with_welcome(
            &mut io,
            &mut DelayProbe::default(),
            &mut DelayProbe::default(),
            config(),
            Some(env!("CARGO_PKG_VERSION")),
            &mut pages,
        )
        .unwrap();

        assert_eq!(startup.settings.welcome_mode, mode);
        assert_eq!(
            io.events
                .iter()
                .filter(|event| matches!(event, Event::St7565Init(_)))
                .count(),
            1
        );
        let display_bytes = io
            .display_trace
            .iter()
            .filter_map(|event| match event {
                DisplayEvent::Write(SPI_WDR, byte) => Some(*byte as u8),
                _ => None,
            })
            .collect::<Vec<_>>();
        let board_bytes = ST7565_COMMANDS
            .into_iter()
            .chain((0..8).flat_map(|page| [0xb0 + page, 0x10, 0x04]))
            .collect::<Vec<_>>();
        assert_eq!(&display_bytes[..board_bytes.len()], board_bytes);

        if matches!(
            mode,
            StartupDisplayMode::None | StartupDisplayMode::FullScreen
        ) {
            assert_eq!(display_bytes.len(), board_bytes.len());
            assert_eq!(pages[6], [0; 128]);
        } else {
            let plan = startup_version_display_plan(mode, Some(env!("CARGO_PKG_VERSION"))).unwrap();
            let StartupVersionDisplayPlan::Text(text) = plan else {
                panic!("text modes must render the brand and project version");
            };
            assert_eq!(
                text.as_bytes(),
                concat!("M0HDY ", env!("CARGO_PKG_VERSION")).as_bytes()
            );
            let mut expected_pages = [[0; 128]; 8];
            plan.render_page(&mut expected_pages).unwrap();
            assert_eq!(pages[6], expected_pages[6]);
            let expected_row = [0x40, 0xb7, 0x10, 0x04]
                .into_iter()
                .chain(expected_pages[6].iter().copied())
                .collect::<Vec<_>>();
            assert_eq!(&display_bytes[board_bytes.len()..], expected_row);
        }

        let last_stop = io
            .events
            .iter()
            .rposition(|event| *event == Event::EepromStop)
            .unwrap();
        let handoff = io
            .events
            .iter()
            .position(|event| *event == Event::I2cHandoff)
            .unwrap();
        let first_battery = io
            .events
            .iter()
            .position(|event| matches!(event, Event::BatteryRead(_)))
            .unwrap();
        assert!(last_stop < handoff && handoff < first_battery);
    }
}

#[test]
fn composed_failures_keep_the_first_error_and_do_not_run_later_effects() {
    let mut io = MockIo::new(Failure::Eeprom);
    let mut pages = [[0; 128]; 8];
    assert!(matches!(
        coordinate_with_welcome(
            &mut io,
            &mut DelayProbe::default(),
            &mut DelayProbe::default(),
            config(),
            Some(env!("CARGO_PKG_VERSION")),
            &mut pages,
        ),
        Err(FirmwareStartupError::Boot(BootError::Settings(_)))
    ));
    assert!(io.events.contains(&Event::I2cHandoff));
    assert!(
        !io.events
            .iter()
            .any(|event| matches!(event, Event::BatteryRead(_) | Event::BatteryWrite(_, _)))
    );
    assert_eq!(
        io.display_trace
            .iter()
            .filter(|event| matches!(event, DisplayEvent::Write(SPI_WDR, _)))
            .count(),
        41
    );

    let mut io = MockIo::new(Failure::Battery);
    assert!(matches!(
        coordinate_with_welcome(
            &mut io,
            &mut DelayProbe::default(),
            &mut DelayProbe::default(),
            config(),
            Some(env!("CARGO_PKG_VERSION")),
            &mut pages,
        ),
        Err(FirmwareStartupError::Boot(BootError::Battery(_)))
    ));
    assert_eq!(
        io.display_trace
            .iter()
            .filter(|event| matches!(event, DisplayEvent::Write(SPI_WDR, _)))
            .count(),
        41
    );
}

#[test]
fn board_failure_stops_before_eeprom_and_battery_and_returns_no_app_state() {
    let mut io = MockIo::new(Failure::Systick);
    let mut delay = DelayProbe::default();
    let mut config = config();
    config.board.enabled_aircopy = true;

    assert_eq!(
        coordinate(&mut io, &mut delay, &mut DelayProbe::default(), config),
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

    let mut config = config();
    config.board.enable_uart = true;
    config.board.enable_fmradio = true;
    assert_eq!(
        coordinate(&mut io, &mut delay, &mut DelayProbe::default(), config),
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
fn crc_truth_table_precedes_settings_and_battery() {
    for uart in [false, true] {
        for aircopy in [false, true] {
            let mut io = MockIo::new(Failure::None);
            let mut delay = DelayProbe::default();
            let mut config = config();
            config.board.enable_fmradio = true;
            config.board.enable_uart = uart;
            config.board.enabled_aircopy = aircopy;
            coordinate(&mut io, &mut delay, &mut DelayProbe::default(), config).unwrap();
            let display = io
                .events
                .iter()
                .position(|event| matches!(event, Event::St7565Init(_)))
                .unwrap();
            let settings = io
                .events
                .iter()
                .position(|event| *event == Event::EepromStart)
                .unwrap();
            let mut expected = vec![
                Event::FmStart,
                Event::FmWrite(0x80),
                Event::FmWrite(0x04),
                Event::FmWrite(0x02),
                Event::FmWrite(0x41),
                Event::FmStop,
                Event::FmRead(GPIOB_DATA),
                Event::FmGpioWrite(GPIOB_DATA, 0x8005),
            ];
            if uart || aircopy {
                expected.extend([
                    Event::CrcWrite(0x4000_3000, 0x100),
                    Event::CrcWrite(0x4000_3004, 0),
                ]);
            }
            assert_eq!(&io.events[display + 1..settings], expected);
        }
    }
}

#[test]
fn fm_failures_preserve_wire_cleanup_gpio_and_abort_boot() {
    let cases = [Failure::FmStart, Failure::FmStop]
        .into_iter()
        .chain((0..4).flat_map(|i| {
            [
                Failure::FmNack(i),
                Failure::FmWrite(i),
                Failure::FmCleanupStop(i),
            ]
        }));
    for failure in cases {
        let mut io = MockIo::new(failure);
        let mut display_delay = DelayProbe::default();
        let mut fm_delay = DelayProbe::default();
        let mut config = config();
        config.board.enable_fmradio = true;
        config.board.enable_uart = true;
        let error = match failure {
            Failure::FmStart => ShutdownError::Transport(FmError::Start),
            Failure::FmStop => ShutdownError::Transport(FmError::Stop),
            Failure::FmNack(_) => ShutdownError::NotAcknowledged,
            Failure::FmWrite(i) | Failure::FmCleanupStop(i) => {
                ShutdownError::Transport(FmError::Write(i))
            }
            _ => unreachable!(),
        };
        assert_eq!(
            coordinate(&mut io, &mut display_delay, &mut fm_delay, config),
            Err(BootError::Board(BoardStartupError::Bk1080(error))),
            "{failure:?}"
        );
        let mut expected = vec![Event::FmStart];
        let count = match failure {
            Failure::FmStart => 0,
            Failure::FmStop => 4,
            Failure::FmNack(i) | Failure::FmWrite(i) | Failure::FmCleanupStop(i) => i + 1,
            _ => unreachable!(),
        };
        expected.extend(
            [0x80, 0x04, 0x02, 0x41]
                .into_iter()
                .take(count)
                .map(Event::FmWrite),
        );
        if failure != Failure::FmStart {
            expected.push(Event::FmStop);
        }
        expected.extend([
            Event::FmRead(GPIOB_DATA),
            Event::FmGpioWrite(GPIOB_DATA, 0x8005),
        ]);
        assert_eq!(&io.events[7..], expected, "{failure:?}");
        assert_eq!(fm_delay.0, [1]);
        assert_eq!(display_delay.0, [1, 20, 120, 120, 1, 1, 40]);
    }
}

#[test]
fn disabled_fm_does_not_use_fm_delay_or_touch_bus_and_gpio() {
    let mut io = MockIo::new(Failure::FmStart);
    let mut display_delay = DelayProbe::default();
    let mut fm_delay = DelayProbe::default();
    coordinate(&mut io, &mut display_delay, &mut fm_delay, config()).unwrap();
    assert!(fm_delay.0.is_empty());
    assert!(!io.events.iter().any(|e| matches!(
        e,
        Event::FmStart | Event::FmRead(_) | Event::FmGpioWrite(_, _)
    )));
}

#[test]
fn eeprom_failure_stops_before_battery_and_returns_no_app_state() {
    let mut io = MockIo::new(Failure::Eeprom);
    let mut delay = DelayProbe::default();

    assert_eq!(
        coordinate(&mut io, &mut delay, &mut DelayProbe::default(), config()),
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
        coordinate(&mut io, &mut delay, &mut DelayProbe::default(), config()),
        Err(BootError::Battery(_))
    ));
    assert_eq!(
        io.events
            .iter()
            .filter(|event| matches!(event, Event::EepromRead(8)))
            .count(),
        3
    );
    assert_eq!(
        io.events
            .iter()
            .filter(|event| matches!(event, Event::BatteryWrite(BatteryWriteRegister::Start, 1)))
            .count(),
        1
    );
}
