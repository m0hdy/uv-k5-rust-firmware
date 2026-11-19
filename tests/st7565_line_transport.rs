#[path = "../src/st7565_line_transport.rs"]
mod st7565_line_transport;

use st7565_line_transport::{A0, DRAIN_POLL_LIMIT, LineError, LineTransport, draw_line};
use std::collections::VecDeque;
use std::num::NonZeroU32;

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Mode(bool),
    A0(A0),
    Fifo(bool),
    Write(u8),
    Drain(bool),
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

fn limit() -> NonZeroU32 {
    NonZeroU32::new(3).unwrap()
}

#[test]
fn immediate_success_follows_literal_trace() {
    let mut mock = Mock::ready();
    assert_eq!(draw_line(&mut mock, 7, 127, &[0xab], limit()), Ok(()));
    assert_eq!(
        mock.events,
        [
            Event::Mode(false),
            Event::A0(A0::Command),
            Event::Fifo(true),
            Event::Write(0xb7),
            Event::Fifo(true),
            Event::Write(0x18),
            Event::Fifo(true),
            Event::Write(0x03),
            Event::Drain(false),
            Event::A0(A0::Data),
            Event::Fifo(true),
            Event::Write(0xab),
            Event::Drain(false),
            Event::Mode(true),
        ]
    );
}

#[test]
fn busy_then_ready_checks_each_byte_and_both_drains() {
    let mut mock = Mock::ready();
    mock.fifo = [false, true, true, false, true, false, true, true].into();
    mock.drain = [true, false, true, true, false].into();
    assert_eq!(draw_line(&mut mock, 0, 0, &[1, 2], limit()), Ok(()));
    assert_eq!(
        mock.events,
        [
            Event::Mode(false),
            Event::A0(A0::Command),
            Event::Fifo(false),
            Event::Fifo(true),
            Event::Write(0xb0),
            Event::Fifo(true),
            Event::Write(0x10),
            Event::Fifo(false),
            Event::Fifo(true),
            Event::Write(0x04),
            Event::Drain(true),
            Event::Drain(false),
            Event::A0(A0::Data),
            Event::Fifo(false),
            Event::Fifo(true),
            Event::Write(1),
            Event::Fifo(true),
            Event::Write(2),
            Event::Drain(true),
            Event::Drain(true),
            Event::Drain(false),
            Event::Mode(true),
        ]
    );
}

#[test]
fn invalid_geometry_never_touches_transport() {
    for (page, column, payload) in [
        (8, 0, &[1][..]),
        (u16::MAX, 0, &[1][..]),
        (0, 128, &[1][..]),
        (0, u16::MAX, &[1][..]),
        (0, 0, &[][..]),
        (0, 127, &[1, 2][..]),
    ] {
        let mut mock = Mock::ready();
        assert_eq!(
            draw_line(&mut mock, page, column, payload, limit()),
            Err(LineError::InvalidGeometry)
        );
        assert!(mock.events.is_empty());
    }
}

#[test]
fn command_fifo_timeout_stops_writes_and_restores_mode() {
    let mut mock = Mock::ready();
    mock.fifo = [true, false, false, false].into();
    assert_eq!(
        draw_line(&mut mock, 0, 0, &[1], limit()),
        Err(LineError::FifoTimeout)
    );
    assert_eq!(
        mock.events,
        [
            Event::Mode(false),
            Event::A0(A0::Command),
            Event::Fifo(true),
            Event::Write(0xb0),
            Event::Fifo(false),
            Event::Fifo(false),
            Event::Fifo(false),
            Event::Mode(true),
        ]
    );
}

#[test]
fn payload_fifo_timeout_stops_writes_and_restores_mode() {
    let mut mock = Mock::ready();
    mock.fifo = [true, true, true, true, false, false, false].into();
    assert_eq!(
        draw_line(&mut mock, 0, 0, &[1, 2], limit()),
        Err(LineError::FifoTimeout)
    );
    assert_eq!(
        mock.events,
        [
            Event::Mode(false),
            Event::A0(A0::Command),
            Event::Fifo(true),
            Event::Write(0xb0),
            Event::Fifo(true),
            Event::Write(0x10),
            Event::Fifo(true),
            Event::Write(0x04),
            Event::Drain(false),
            Event::A0(A0::Data),
            Event::Fifo(true),
            Event::Write(1),
            Event::Fifo(false),
            Event::Fifo(false),
            Event::Fifo(false),
            Event::Mode(true),
        ]
    );
}

#[test]
fn command_drain_timeout_stops_before_data_and_restores_mode() {
    let mut mock = Mock::ready();
    mock.default_busy = true;
    assert_eq!(
        draw_line(&mut mock, 0, 0, &[1], limit()),
        Err(LineError::DrainTimeout)
    );
    assert_eq!(
        &mock.events[..8],
        [
            Event::Mode(false),
            Event::A0(A0::Command),
            Event::Fifo(true),
            Event::Write(0xb0),
            Event::Fifo(true),
            Event::Write(0x10),
            Event::Fifo(true),
            Event::Write(0x04),
        ]
    );
    assert_eq!(mock.events.len(), 8 + DRAIN_POLL_LIMIT as usize + 1);
    assert!(
        mock.events[8..mock.events.len() - 1]
            .iter()
            .all(|e| *e == Event::Drain(true))
    );
    assert_eq!(mock.events.last(), Some(&Event::Mode(true)));
}

#[test]
fn final_drain_timeout_restores_mode() {
    let mut mock = Mock::ready();
    mock.drain = [false].into();
    mock.default_busy = true;
    assert_eq!(
        draw_line(&mut mock, 0, 0, &[1], limit()),
        Err(LineError::DrainTimeout)
    );
    assert_eq!(
        &mock.events[..12],
        [
            Event::Mode(false),
            Event::A0(A0::Command),
            Event::Fifo(true),
            Event::Write(0xb0),
            Event::Fifo(true),
            Event::Write(0x10),
            Event::Fifo(true),
            Event::Write(0x04),
            Event::Drain(false),
            Event::A0(A0::Data),
            Event::Fifo(true),
            Event::Write(1),
        ]
    );
    assert_eq!(mock.events.len(), 12 + DRAIN_POLL_LIMIT as usize + 1);
    assert!(
        mock.events[12..mock.events.len() - 1]
            .iter()
            .all(|e| *e == Event::Drain(true))
    );
    assert_eq!(mock.events.last(), Some(&Event::Mode(true)));
}

#[test]
fn last_permitted_drain_read_can_succeed() {
    let mut mock = Mock::ready();
    mock.drain = std::iter::repeat_n(true, DRAIN_POLL_LIMIT as usize - 1)
        .chain([false, false])
        .collect();
    assert_eq!(draw_line(&mut mock, 0, 0, &[1], limit()), Ok(()));
    assert_eq!(
        mock.events[8..8 + DRAIN_POLL_LIMIT as usize]
            .iter()
            .filter(|e| **e == Event::Drain(true))
            .count(),
        DRAIN_POLL_LIMIT as usize - 1
    );
    assert_eq!(
        mock.events[8 + DRAIN_POLL_LIMIT as usize - 1],
        Event::Drain(false)
    );
    assert_eq!(
        &mock.events[mock.events.len() - 2..],
        &[Event::Drain(false), Event::Mode(true)]
    );
}
