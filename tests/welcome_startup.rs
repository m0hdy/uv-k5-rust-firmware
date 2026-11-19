use std::collections::BTreeMap;
use std::num::NonZeroU32;

use uv_k5_rust_firmware::st7565_line_transport::{DRAIN_POLL_LIMIT, LineError};
use uv_k5_rust_firmware::st7565_startup_executor::target_spi0::{
    DelayMs, RegisterIo, startup_with_io,
};
use uv_k5_rust_firmware::startup_version_display::{
    LCD_WIDTH, STARTUP_VERSION_LINE, StartupDisplayMode, StartupVersionError,
    StartupVersionRenderError,
};
use uv_k5_rust_firmware::startup_version_row_plan::StartupVersionRowError;
use uv_k5_rust_firmware::welcome_startup::{
    WelcomeError, welcome_on_initialized_display_with_io,
    welcome_on_initialized_display_with_version_and_io, welcome_with_io,
    welcome_with_version_and_io,
};

const CR: usize = 0x400b_8000;
const WDR: usize = 0x400b_8004;
const IF: usize = 0x400b_8014;
const FIFOST: usize = 0x400b_8018;
const GPIOB: usize = 0x4006_0800;
const MODE: u32 = 1 << 12;
const A0: u32 = 1 << 9;
const STARTUP_BYTES: usize = 41;

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Read(usize, u32),
    Write(usize, u32),
}

#[derive(Default)]
struct Registers {
    values: BTreeMap<usize, u32>,
    events: Vec<Event>,
    fifo_reads: usize,
    drain_reads: usize,
    fifo_fail_after: Option<usize>,
    drain_fail_after: Option<usize>,
}

impl Registers {
    fn bytes(&self) -> Vec<(bool, u8)> {
        let mut gpio = 0;
        let mut bytes = Vec::new();
        for event in &self.events {
            match *event {
                Event::Write(GPIOB, value) => gpio = value,
                Event::Write(WDR, value) => bytes.push((gpio & A0 != 0, value as u8)),
                _ => {}
            }
        }
        bytes
    }

    fn assert_restored(&self) {
        assert!(matches!(
            self.events.last(),
            Some(Event::Write(CR, value)) if value & MODE != 0
        ));
    }
}

impl RegisterIo for Registers {
    fn read(&mut self, address: usize) -> u32 {
        let value = match address {
            FIFOST => {
                let count = self.fifo_reads;
                self.fifo_reads += 1;
                if self.fifo_fail_after.is_some_and(|limit| count >= limit) {
                    1 << 4
                } else {
                    0
                }
            }
            IF => {
                let count = self.drain_reads;
                self.drain_reads += 1;
                if self.drain_fail_after.is_some_and(|limit| count >= limit) {
                    0x20
                } else {
                    0
                }
            }
            _ => *self.values.get(&address).unwrap_or(&0),
        };
        if address != FIFOST && address != IF {
            self.events.push(Event::Read(address, value));
        }
        value
    }

    fn write(&mut self, address: usize, value: u32) {
        self.values.insert(address, value);
        self.events.push(Event::Write(address, value));
    }
}

#[derive(Default, Debug, PartialEq, Eq)]
struct Delay(Vec<u16>);

impl DelayMs for Delay {
    fn delay_ms(&mut self, milliseconds: u16) {
        self.0.push(milliseconds);
    }
}

fn limit() -> NonZeroU32 {
    NonZeroU32::new(3).unwrap()
}

fn pages() -> [[u8; LCD_WIDTH]; 7] {
    std::array::from_fn(|page| std::array::from_fn(|column| (page * 17 + column) as u8))
}

fn golden_dev() -> [[u8; LCD_WIDTH]; 7] {
    let mut expected = pages();
    let row = &mut expected[STARTUP_VERSION_LINE];
    for (start, glyph) in [
        (34, [0x7f, 0x02, 0x04, 0x04, 0x02, 0x7f]),
        (41, [0x3e, 0x41, 0x41, 0x41, 0x41, 0x3e]),
        (48, [0x7f, 0x08, 0x08, 0x08, 0x08, 0x7f]),
        (55, [0x7f, 0x41, 0x41, 0x41, 0x41, 0x3e]),
        (62, [0x07, 0x08, 0x70, 0x08, 0x07, 0x00]),
        (76, [0x38, 0x44, 0x44, 0x44, 0x7f, 0x00]),
        (83, [0x38, 0x54, 0x54, 0x54, 0x48, 0x00]),
        (90, [0x0c, 0x30, 0x40, 0x30, 0x0c, 0x00]),
    ] {
        row[start..start + 6].copy_from_slice(&glyph);
    }
    expected
}

#[test]
fn text_modes_initialize_then_send_exact_golden_row() {
    let mut startup = Registers::default();
    let mut startup_delay = Delay::default();
    startup_with_io(&mut startup, &mut startup_delay, limit()).unwrap();

    for mode in [StartupDisplayMode::Voltage, StartupDisplayMode::Custom] {
        let mut io = Registers::default();
        let mut delay = Delay::default();
        let mut frame = pages();
        welcome_with_version_and_io(&mut io, &mut delay, mode, Some("dev"), &mut frame, limit())
            .unwrap();
        assert_eq!(&io.events[..startup.events.len()], startup.events);
        assert_eq!(delay, startup_delay);
        let bytes = io.bytes();
        assert_eq!(&bytes[..STARTUP_BYTES], startup.bytes());
        assert_eq!(
            &bytes[STARTUP_BYTES..STARTUP_BYTES + 4],
            &[(false, 0x40), (false, 0xb7), (false, 0x10), (false, 0x04)]
        );
        assert_eq!(
            &bytes[STARTUP_BYTES + 4..],
            golden_dev()[6].map(|byte| (true, byte))
        );
        let control = startup.values[&CR];
        let gpio = startup.values[&GPIOB];
        let mut row_events = vec![
            Event::Read(CR, control),
            Event::Write(CR, control & !MODE),
            Event::Read(GPIOB, gpio),
            Event::Write(GPIOB, gpio & !A0),
        ];
        row_events.extend(
            [0x40, 0xb7, 0x10, 0x04]
                .into_iter()
                .map(|byte| Event::Write(WDR, byte)),
        );
        row_events.extend([
            Event::Read(GPIOB, gpio & !A0),
            Event::Write(GPIOB, gpio | A0),
        ]);
        row_events.extend(golden_dev()[6].map(|byte| Event::Write(WDR, u32::from(byte))));
        row_events.extend([
            Event::Read(CR, control & !MODE),
            Event::Write(CR, control | MODE),
        ]);
        assert_eq!(&io.events[startup.events.len()..], row_events);
        assert_eq!(frame, golden_dev());
        assert_eq!(io.drain_reads, 19);
        io.assert_restored();
    }
}

#[test]
fn suppressed_modes_only_initialize_even_with_invalid_unused_version_and_no_pages() {
    let mut startup = Registers::default();
    let mut startup_delay = Delay::default();
    startup_with_io(&mut startup, &mut startup_delay, limit()).unwrap();
    for mode in [StartupDisplayMode::None, StartupDisplayMode::FullScreen] {
        let mut io = Registers::default();
        let mut delay = Delay::default();
        let mut frame = pages();
        welcome_with_version_and_io(&mut io, &mut delay, mode, Some("é"), &mut frame, limit())
            .unwrap();
        assert_eq!(io.events, startup.events);
        assert_eq!(delay, startup_delay);
        assert_eq!(frame, pages());
        let mut empty = [];
        welcome_with_version_and_io(
            &mut Registers::default(),
            &mut Delay::default(),
            mode,
            Some(""),
            &mut empty,
            limit(),
        )
        .unwrap();
    }
}

#[test]
fn invalid_input_fails_without_io_delays_or_frame_changes() {
    for (version, error) in [
        ("", StartupVersionError::EmptyVersion),
        ("é", StartupVersionError::UnsupportedCharacter),
        ("1234567890123", StartupVersionError::TextTooWide),
    ] {
        let mut io = Registers::default();
        let mut delay = Delay::default();
        let mut frame = pages();
        assert_eq!(
            welcome_with_version_and_io(
                &mut io,
                &mut delay,
                StartupDisplayMode::Custom,
                Some(version),
                &mut frame,
                limit()
            ),
            Err(WelcomeError::Version(error))
        );
        assert!(io.events.is_empty());
        assert!(delay.0.is_empty());
        assert_eq!(frame, pages());
    }
    let mut io = Registers::default();
    let mut delay = Delay::default();
    let mut frame = pages();
    assert_eq!(
        welcome_with_version_and_io(
            &mut io,
            &mut delay,
            StartupDisplayMode::Voltage,
            Some("dev"),
            &mut frame[..6],
            limit()
        ),
        Err(WelcomeError::Row(StartupVersionRowError::Render(
            StartupVersionRenderError::MissingPage
        )))
    );
    assert!(io.events.is_empty());
    assert!(delay.0.is_empty());
    assert_eq!(frame, pages());
}

#[test]
fn convenience_entry_uses_package_version() {
    let mut actual = pages();
    let mut explicit = pages();
    let mut io = Registers::default();
    let mut reference = Registers::default();
    welcome_with_io(
        &mut io,
        &mut Delay::default(),
        StartupDisplayMode::Custom,
        &mut actual,
        limit(),
    )
    .unwrap();
    welcome_with_version_and_io(
        &mut reference,
        &mut Delay::default(),
        StartupDisplayMode::Custom,
        Some(env!("CARGO_PKG_VERSION")),
        &mut explicit,
        limit(),
    )
    .unwrap();
    assert_eq!(io.events, reference.events);
    assert_eq!(actual, explicit);
}

#[test]
fn fifo_timeouts_short_circuit_initialization_and_row_and_restore_mode() {
    for failed_byte in [0, 17, 40, 41, 43, 45, 100, 172] {
        let mut io = Registers {
            fifo_fail_after: Some(failed_byte),
            ..Registers::default()
        };
        let error = welcome_with_version_and_io(
            &mut io,
            &mut Delay::default(),
            StartupDisplayMode::Custom,
            Some("dev"),
            &mut pages(),
            limit(),
        )
        .unwrap_err();
        let expected = if failed_byte < STARTUP_BYTES {
            WelcomeError::Initialization(LineError::FifoTimeout)
        } else {
            WelcomeError::Transfer(LineError::FifoTimeout)
        };
        assert_eq!(error, expected);
        assert_eq!(io.bytes().len(), failed_byte);
        assert_eq!(io.fifo_reads, failed_byte + limit().get() as usize);
        io.assert_restored();
    }
}

#[test]
fn drain_timeouts_short_circuit_initialization_and_row_and_restore_mode() {
    for failed_drain in [0, 1, 16, 17, 18] {
        let mut io = Registers {
            drain_fail_after: Some(failed_drain),
            ..Registers::default()
        };
        let error = welcome_with_version_and_io(
            &mut io,
            &mut Delay::default(),
            StartupDisplayMode::Custom,
            Some("dev"),
            &mut pages(),
            limit(),
        )
        .unwrap_err();
        let expected = if failed_drain < 17 {
            WelcomeError::Initialization(LineError::DrainTimeout)
        } else {
            WelcomeError::Transfer(LineError::DrainTimeout)
        };
        assert_eq!(error, expected);
        assert_eq!(io.drain_reads, failed_drain + DRAIN_POLL_LIMIT as usize);
        let byte_count = match failed_drain {
            0 => 17,
            1 => 20,
            16 => 41,
            17 => 45,
            18 => 173,
            _ => unreachable!(),
        };
        assert_eq!(io.bytes().len(), byte_count);
        io.assert_restored();
    }
}

#[test]
fn initialized_text_modes_transfer_only_the_exact_golden_row() {
    for mode in [StartupDisplayMode::Custom, StartupDisplayMode::Voltage] {
        let mut io = Registers::default();
        let mut frame = pages();
        welcome_on_initialized_display_with_version_and_io(
            &mut io,
            mode,
            Some("dev"),
            &mut frame,
            limit(),
        )
        .unwrap();
        let mut expected = vec![
            Event::Read(CR, 0),
            Event::Write(CR, 0),
            Event::Read(GPIOB, 0),
            Event::Write(GPIOB, 0),
        ];
        expected.extend([0x40, 0xb7, 0x10, 0x04].map(|byte| Event::Write(WDR, byte)));
        expected.extend([Event::Read(GPIOB, 0), Event::Write(GPIOB, A0)]);
        expected.extend(golden_dev()[6].map(|byte| Event::Write(WDR, u32::from(byte))));
        expected.extend([Event::Read(CR, 0), Event::Write(CR, MODE)]);
        assert_eq!(io.events, expected);
        assert_eq!(io.fifo_reads, 132);
        assert_eq!(io.drain_reads, 2);
        assert_eq!(frame, golden_dev());
    }
}

#[test]
fn initialized_suppressed_and_invalid_inputs_have_no_display_effects() {
    for mode in [StartupDisplayMode::None, StartupDisplayMode::FullScreen] {
        let mut io = Registers::default();
        let mut frame = pages();
        welcome_on_initialized_display_with_version_and_io(
            &mut io,
            mode,
            Some("é"),
            &mut frame,
            limit(),
        )
        .unwrap();
        assert!(io.events.is_empty());
        assert_eq!(frame, pages());
        welcome_on_initialized_display_with_version_and_io(
            &mut io,
            mode,
            Some(""),
            &mut [],
            limit(),
        )
        .unwrap();
    }
    let mut io = Registers::default();
    let mut frame = pages();
    assert_eq!(
        welcome_on_initialized_display_with_version_and_io(
            &mut io,
            StartupDisplayMode::Custom,
            Some("é"),
            &mut frame,
            limit()
        ),
        Err(WelcomeError::Version(
            StartupVersionError::UnsupportedCharacter
        ))
    );
    assert_eq!(
        welcome_on_initialized_display_with_version_and_io(
            &mut io,
            StartupDisplayMode::Voltage,
            Some("dev"),
            &mut frame[..6],
            limit()
        ),
        Err(WelcomeError::Row(StartupVersionRowError::Render(
            StartupVersionRenderError::MissingPage
        )))
    );
    assert!(io.events.is_empty());
    assert_eq!(frame, pages());
}

#[test]
fn initialized_package_entry_uses_package_version() {
    let mut actual = pages();
    let mut explicit = pages();
    let mut io = Registers::default();
    let mut reference = Registers::default();
    welcome_on_initialized_display_with_io(
        &mut io,
        StartupDisplayMode::Custom,
        &mut actual,
        limit(),
    )
    .unwrap();
    welcome_on_initialized_display_with_version_and_io(
        &mut reference,
        StartupDisplayMode::Custom,
        Some(env!("CARGO_PKG_VERSION")),
        &mut explicit,
        limit(),
    )
    .unwrap();
    assert_eq!(io.events, reference.events);
    assert_eq!(actual, explicit);
}

#[test]
fn initialized_transfer_timeouts_are_bounded_and_restore_mode() {
    for failed_byte in [0, 3, 4, 59, 131] {
        let mut io = Registers {
            fifo_fail_after: Some(failed_byte),
            ..Registers::default()
        };
        assert_eq!(
            welcome_on_initialized_display_with_version_and_io(
                &mut io,
                StartupDisplayMode::Custom,
                Some("dev"),
                &mut pages(),
                limit()
            ),
            Err(WelcomeError::Transfer(LineError::FifoTimeout))
        );
        assert_eq!(io.bytes().len(), failed_byte);
        assert_eq!(io.fifo_reads, failed_byte + limit().get() as usize);
        io.assert_restored();
    }
    for failed_drain in [0, 1] {
        let mut io = Registers {
            drain_fail_after: Some(failed_drain),
            ..Registers::default()
        };
        assert_eq!(
            welcome_on_initialized_display_with_version_and_io(
                &mut io,
                StartupDisplayMode::Voltage,
                Some("dev"),
                &mut pages(),
                limit()
            ),
            Err(WelcomeError::Transfer(LineError::DrainTimeout))
        );
        assert_eq!(io.drain_reads, failed_drain + DRAIN_POLL_LIMIT as usize);
        assert_eq!(io.bytes().len(), if failed_drain == 0 { 4 } else { 132 });
        io.assert_restored();
    }
}
