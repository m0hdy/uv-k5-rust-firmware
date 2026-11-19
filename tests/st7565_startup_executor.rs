use std::collections::VecDeque;
use std::num::NonZeroU32;

use uv_k5_rust_firmware::st7565_line_transport::{A0, DRAIN_POLL_LIMIT, LineError, LineTransport};
use uv_k5_rust_firmware::st7565_startup::ResetLevel;
use uv_k5_rust_firmware::st7565_startup_executor::{StartupTransport, execute_startup};

#[derive(Debug, PartialEq, Eq)]
enum Event {
    InitializeSpi0,
    Reset(ResetLevel),
    DelayMs(u16),
    Mode(bool),
    A0(A0),
    Fifo(bool),
    Write(u8),
    Drain(bool),
    FillScreenHandoff(u8),
}

#[derive(Default)]
struct Mock {
    events: Vec<Event>,
    fifo: VecDeque<bool>,
    drain: VecDeque<bool>,
    default_fifo: bool,
    default_busy: bool,
}

impl Mock {
    fn ready() -> Self {
        Self {
            default_fifo: true,
            ..Self::default()
        }
    }
}

impl LineTransport for Mock {
    fn set_spi_mode(&mut self, enabled: bool) {
        self.events.push(Event::Mode(enabled));
    }

    fn set_a0(&mut self, a0: A0) {
        self.events.push(Event::A0(a0));
    }

    fn fifo_ready(&mut self) -> bool {
        let ready = self.fifo.pop_front().unwrap_or(self.default_fifo);
        self.events.push(Event::Fifo(ready));
        ready
    }

    fn write_byte(&mut self, byte: u8) {
        self.events.push(Event::Write(byte));
    }

    fn drain_busy(&mut self) -> bool {
        let busy = self.drain.pop_front().unwrap_or(self.default_busy);
        self.events.push(Event::Drain(busy));
        busy
    }
}

impl StartupTransport for Mock {
    fn initialize_spi0(&mut self) {
        self.events.push(Event::InitializeSpi0);
    }

    fn set_reset(&mut self, level: ResetLevel) {
        self.events.push(Event::Reset(level));
    }

    fn delay_ms(&mut self, milliseconds: u16) {
        self.events.push(Event::DelayMs(milliseconds));
    }

    fn fill_screen_handoff(&mut self, fill: u8) {
        self.events.push(Event::FillScreenHandoff(fill));
    }
}

fn limit() -> NonZeroU32 {
    NonZeroU32::new(3).unwrap()
}

#[test]
fn startup_runs_as_one_chronological_transport_trace() {
    let mut mock = Mock::ready();
    assert_eq!(execute_startup(&mut mock, limit()), Ok(()));

    let mut expected = vec![
        Event::InitializeSpi0,
        Event::Reset(ResetLevel::High),
        Event::DelayMs(1),
        Event::Reset(ResetLevel::Low),
        Event::DelayMs(20),
        Event::Reset(ResetLevel::High),
        Event::DelayMs(120),
        Event::Mode(false),
    ];
    for (byte, preceding_delay) in [
        (0xe2, None),
        (0xa2, Some(120)),
        (0xc0, None),
        (0xa1, None),
        (0xa6, None),
        (0xa4, None),
        (0x24, None),
        (0x81, None),
        (0x1f, None),
        (0x2b, None),
        (0x2e, Some(1)),
        (0x2f, Some(1)),
        (0x2f, None),
        (0x2f, None),
        (0x2f, None),
        (0x40, Some(40)),
        (0xaf, None),
    ] {
        if let Some(milliseconds) = preceding_delay {
            expected.push(Event::DelayMs(milliseconds));
        }
        expected.extend([
            Event::A0(A0::Command),
            Event::Fifo(true),
            Event::Write(byte),
        ]);
    }
    expected.extend([
        Event::Drain(false),
        Event::Mode(true),
        Event::FillScreenHandoff(0),
    ]);

    assert_eq!(mock.events, expected);
}

#[test]
fn fifo_ready_on_last_permitted_poll_completes_startup() {
    let mut mock = Mock::ready();
    mock.fifo = [false, false, true].into();
    assert_eq!(execute_startup(&mut mock, limit()), Ok(()));
    assert_eq!(
        &mock.events[7..13],
        [
            Event::Mode(false),
            Event::A0(A0::Command),
            Event::Fifo(false),
            Event::Fifo(false),
            Event::Fifo(true),
            Event::Write(0xe2),
        ]
    );
    assert_eq!(mock.events.last(), Some(&Event::FillScreenHandoff(0)));
}

#[test]
fn fifo_timeout_restores_mode_and_skips_remaining_effects() {
    let mut mock = Mock {
        default_fifo: false,
        ..Mock::default()
    };
    assert_eq!(
        execute_startup(&mut mock, limit()),
        Err(LineError::FifoTimeout)
    );
    assert_eq!(
        mock.events,
        [
            Event::InitializeSpi0,
            Event::Reset(ResetLevel::High),
            Event::DelayMs(1),
            Event::Reset(ResetLevel::Low),
            Event::DelayMs(20),
            Event::Reset(ResetLevel::High),
            Event::DelayMs(120),
            Event::Mode(false),
            Event::A0(A0::Command),
            Event::Fifo(false),
            Event::Fifo(false),
            Event::Fifo(false),
            Event::Mode(true),
        ]
    );
}

#[test]
fn drain_timeout_restores_mode_and_skips_fill_handoff() {
    let mut mock = Mock {
        default_fifo: true,
        default_busy: true,
        ..Mock::default()
    };
    assert_eq!(
        execute_startup(&mut mock, limit()),
        Err(LineError::DrainTimeout)
    );
    assert_eq!(
        mock.events
            .iter()
            .filter(|event| matches!(event, Event::Drain(true)))
            .count(),
        DRAIN_POLL_LIMIT as usize
    );
    assert_eq!(mock.events.last(), Some(&Event::Mode(true)));
    assert!(!mock.events.contains(&Event::FillScreenHandoff(0)));
}
