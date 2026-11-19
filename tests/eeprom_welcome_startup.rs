use std::cell::RefCell;
use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::rc::Rc;

use uv_k5_rust_firmware::eeprom_read_transport::{Acknowledgement, I2cTransport, ReadError};
use uv_k5_rust_firmware::eeprom_welcome_startup::{
    EepromWelcomeError, welcome_with_io, welcome_with_version_and_io,
};
use uv_k5_rust_firmware::st7565_line_transport::LineError;
use uv_k5_rust_firmware::st7565_startup_executor::target_spi0::{DelayMs, RegisterIo};
use uv_k5_rust_firmware::startup_version_display::{
    LCD_WIDTH, StartupDisplayMode, StartupVersionError, StartupVersionRenderError,
};
use uv_k5_rust_firmware::startup_version_row_plan::StartupVersionRowError;
use uv_k5_rust_firmware::welcome_startup::{self, WelcomeError};

#[derive(Clone, Debug, Eq, PartialEq)]
enum Event {
    Start,
    Restart,
    Byte(u8),
    Read(usize),
    Stop,
    DisplayRead(usize),
    DisplayWrite(usize, u32),
    Delay(u16),
}

type Trace = Rc<RefCell<Vec<Event>>>;

struct Bus {
    trace: Trace,
    mode: u8,
    calls: usize,
    fail: Option<usize>,
    nack: Option<usize>,
    cleanup_fails: bool,
}

impl Bus {
    fn new(trace: Trace, mode: u8) -> Self {
        Self {
            trace,
            mode,
            calls: 0,
            fail: None,
            nack: None,
            cleanup_fails: false,
        }
    }

    fn call(&mut self, event: Event) -> Result<(), u8> {
        self.trace.borrow_mut().push(event);
        let call = self.calls;
        self.calls += 1;
        if self.fail == Some(call) {
            Err(7)
        } else {
            Ok(())
        }
    }
}

impl I2cTransport for Bus {
    type Error = u8;

    fn start(&mut self) -> Result<(), u8> {
        self.call(Event::Start)
    }
    fn repeated_start(&mut self) -> Result<(), u8> {
        self.call(Event::Restart)
    }
    fn write_byte(&mut self, byte: u8) -> Result<Acknowledgement, u8> {
        let call = self.calls;
        self.call(Event::Byte(byte))?;
        Ok(if self.nack == Some(call) {
            Acknowledgement::Nack
        } else {
            Acknowledgement::Ack
        })
    }
    fn read_buffer(&mut self, bytes: &mut [u8]) -> Result<(), u8> {
        bytes.fill(0xff);
        bytes[7] = self.mode;
        self.call(Event::Read(bytes.len()))
    }
    fn stop(&mut self) -> Result<(), u8> {
        self.call(Event::Stop)?;
        if self.cleanup_fails { Err(9) } else { Ok(()) }
    }
}

#[derive(Default)]
struct Display {
    trace: Trace,
    values: BTreeMap<usize, u32>,
    bytes: usize,
    fail_fifo: Option<usize>,
    drains: usize,
    fail_drain: Option<usize>,
}

impl RegisterIo for Display {
    fn read(&mut self, address: usize) -> u32 {
        self.trace.borrow_mut().push(Event::DisplayRead(address));
        match address {
            0x400b_8018 => u32::from(self.fail_fifo == Some(self.bytes)) << 4,
            0x400b_8014 => {
                if self.fail_drain == Some(self.drains) {
                    0x20
                } else {
                    self.drains += 1;
                    0
                }
            }
            _ => *self.values.get(&address).unwrap_or(&0),
        }
    }
    fn write(&mut self, address: usize, value: u32) {
        self.trace
            .borrow_mut()
            .push(Event::DisplayWrite(address, value));
        self.values.insert(address, value);
        if address == 0x400b_8004 {
            self.bytes += 1;
        }
    }
}

struct Delay(Trace);
impl DelayMs for Delay {
    fn delay_ms(&mut self, ms: u16) {
        self.0.borrow_mut().push(Event::Delay(ms));
    }
}

fn limit() -> NonZeroU32 {
    NonZeroU32::new(3).unwrap()
}

fn transaction() -> Vec<Event> {
    vec![
        Event::Start,
        Event::Byte(0xa0),
        Event::Byte(0x0e),
        Event::Byte(0x90),
        Event::Restart,
        Event::Byte(0xa1),
        Event::Read(8),
        Event::Stop,
    ]
}

#[test]
fn encodings_and_invalid_defaults_execute_exact_concrete_welcome_after_stop() {
    for (raw, mode) in [
        (0, StartupDisplayMode::FullScreen),
        (1, StartupDisplayMode::Custom),
        (2, StartupDisplayMode::Voltage),
        (3, StartupDisplayMode::None),
        (4, StartupDisplayMode::Voltage),
        (255, StartupDisplayMode::Voltage),
    ] {
        let trace = Trace::default();
        let mut bus = Bus::new(trace.clone(), raw);
        let mut display = Display {
            trace: trace.clone(),
            ..Display::default()
        };
        let mut pages = [[0xa5; LCD_WIDTH]; 7];
        welcome_with_version_and_io(
            &mut bus,
            &mut display,
            &mut Delay(trace.clone()),
            Some("dev"),
            &mut pages,
            limit(),
        )
        .unwrap();

        let reference = Trace::default();
        let mut expected_display = Display {
            trace: reference.clone(),
            ..Display::default()
        };
        let mut expected_pages = [[0xa5; LCD_WIDTH]; 7];
        welcome_startup::welcome_with_version_and_io(
            &mut expected_display,
            &mut Delay(reference.clone()),
            mode,
            Some("dev"),
            &mut expected_pages,
            limit(),
        )
        .unwrap();
        let mut expected = transaction();
        expected.extend(reference.borrow().iter().cloned());
        assert_eq!(*trace.borrow(), expected, "encoding {raw}");
        assert_eq!(pages, expected_pages);
        assert_eq!(display.bytes, if [0, 3].contains(&raw) { 41 } else { 173 });
        if [0, 3].contains(&raw) {
            assert_eq!(pages, [[0xa5; LCD_WIDTH]; 7]);
        } else {
            assert_ne!(pages[6], [0xa5; LCD_WIDTH]);
        }
    }
}

#[test]
fn package_version_entry_matches_existing_package_welcome() {
    let trace = Trace::default();
    let mut actual = [[0; LCD_WIDTH]; 7];
    let mut display = Display {
        trace: trace.clone(),
        ..Display::default()
    };
    welcome_with_io(
        &mut Bus::new(trace.clone(), 1),
        &mut display,
        &mut Delay(trace.clone()),
        &mut actual,
        limit(),
    )
    .unwrap();
    let reference = Trace::default();
    let mut expected = [[0; LCD_WIDTH]; 7];
    welcome_startup::welcome_with_io(
        &mut Display {
            trace: reference.clone(),
            ..Display::default()
        },
        &mut Delay(reference.clone()),
        StartupDisplayMode::Custom,
        &mut expected,
        limit(),
    )
    .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(&trace.borrow()[8..], reference.borrow().as_slice());
}

#[test]
fn every_acquisition_failure_stops_without_display_delays_or_page_changes() {
    let trace = Trace::default();
    let mut bus = Bus::new(trace.clone(), 1);
    bus.cleanup_fails = true;
    let mut pages = [[0xa5; LCD_WIDTH]; 7];
    assert_eq!(
        welcome_with_io(
            &mut bus,
            &mut Display {
                trace: trace.clone(),
                ..Display::default()
            },
            &mut Delay(trace.clone()),
            &mut pages,
            limit(),
        ),
        Err(EepromWelcomeError::Acquisition(ReadError::Transport(9)))
    );
    assert_eq!(*trace.borrow(), transaction());
    assert_eq!(pages, [[0xa5; LCD_WIDTH]; 7]);

    for fail in 0..8 {
        for cleanup_fails in [false, true] {
            let trace = Trace::default();
            let mut bus = Bus::new(trace.clone(), 1);
            bus.fail = Some(fail);
            bus.cleanup_fails = cleanup_fails;
            let mut pages = [[0xa5; LCD_WIDTH]; 7];
            assert_eq!(
                welcome_with_io(
                    &mut bus,
                    &mut Display {
                        trace: trace.clone(),
                        ..Display::default()
                    },
                    &mut Delay(trace.clone()),
                    &mut pages,
                    limit(),
                ),
                Err(EepromWelcomeError::Acquisition(ReadError::Transport(7)))
            );
            let mut expected = transaction()[..=fail].to_vec();
            if fail != 0 && fail != 7 {
                expected.push(Event::Stop);
            }
            assert_eq!(*trace.borrow(), expected);
            assert_eq!(pages, [[0xa5; LCD_WIDTH]; 7]);
        }
    }
    for nack in [1, 2, 3, 5] {
        let trace = Trace::default();
        let mut bus = Bus::new(trace.clone(), 1);
        bus.nack = Some(nack);
        bus.cleanup_fails = true;
        let mut pages = [[0xa5; LCD_WIDTH]; 7];
        assert_eq!(
            welcome_with_io(
                &mut bus,
                &mut Display {
                    trace: trace.clone(),
                    ..Display::default()
                },
                &mut Delay(trace.clone()),
                &mut pages,
                limit(),
            ),
            Err(EepromWelcomeError::Acquisition(ReadError::NotAcknowledged))
        );
        let mut expected = transaction()[..=nack].to_vec();
        expected.push(Event::Stop);
        assert_eq!(*trace.borrow(), expected);
        assert_eq!(pages, [[0xa5; LCD_WIDTH]; 7]);
    }
}

#[test]
fn successful_read_preserves_concrete_welcome_errors_and_short_circuiting() {
    for (version, page_count, fifo, drain, expected) in [
        (
            Some("é"),
            7,
            None,
            None,
            WelcomeError::Version(StartupVersionError::UnsupportedCharacter),
        ),
        (
            Some("dev"),
            6,
            None,
            None,
            WelcomeError::Row(StartupVersionRowError::Render(
                StartupVersionRenderError::MissingPage,
            )),
        ),
        (
            Some("dev"),
            7,
            Some(0),
            None,
            WelcomeError::Initialization(LineError::FifoTimeout),
        ),
        (
            Some("dev"),
            7,
            Some(41),
            None,
            WelcomeError::Transfer(LineError::FifoTimeout),
        ),
        (
            Some("dev"),
            7,
            None,
            Some(0),
            WelcomeError::Initialization(LineError::DrainTimeout),
        ),
        (
            Some("dev"),
            7,
            None,
            Some(17),
            WelcomeError::Transfer(LineError::DrainTimeout),
        ),
    ] {
        let trace = Trace::default();
        let mut display = Display {
            trace: trace.clone(),
            fail_fifo: fifo,
            fail_drain: drain,
            ..Display::default()
        };
        let mut pages = [[0xa5; LCD_WIDTH]; 7];
        assert_eq!(
            welcome_with_version_and_io(
                &mut Bus::new(trace.clone(), 1),
                &mut display,
                &mut Delay(trace.clone()),
                version,
                &mut pages[..page_count],
                limit(),
            ),
            Err(EepromWelcomeError::Welcome(expected))
        );
        assert_eq!(&trace.borrow()[..8], transaction());
        if matches!(expected, WelcomeError::Version(_) | WelcomeError::Row(_)) {
            assert_eq!(*trace.borrow(), transaction());
            assert_eq!(pages, [[0xa5; LCD_WIDTH]; 7]);
        } else {
            assert_eq!(
                display.bytes,
                fifo.unwrap_or(if drain == Some(0) { 17 } else { 45 })
            );
            assert_ne!(pages[6], [0xa5; LCD_WIDTH]);
            assert!(
                matches!(trace.borrow().last(), Some(Event::DisplayWrite(0x400b_8000, value)) if value & (1 << 12) != 0)
            );
        }
    }
}
