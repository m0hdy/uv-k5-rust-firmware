#[path = "../src/uart_reply_executor.rs"]
mod uart_reply_executor;

use std::collections::VecDeque;
use std::num::NonZeroU32;
use uart_reply_executor::{
    FifoBackend, ReplyError, SendPhase, UART_IF_TXFIFO_FULL_MASK, UART1_BASE, UART1_IF, UART1_TDR,
    send_reply,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Boundary(SendPhase),
    Write(u8),
    If(u32),
}

#[derive(Default)]
struct Mock {
    events: Vec<Event>,
    if_values: VecDeque<u32>,
    default_if: u32,
    first_reply_at_send: Option<Vec<u8>>,
}

impl FifoBackend for Mock {
    fn begin_uart_send(&mut self, phase: SendPhase, reply_payload: &[u8]) {
        if self.first_reply_at_send.is_none() {
            self.first_reply_at_send = Some(reply_payload.to_vec());
        }
        self.events.push(Event::Boundary(phase));
    }

    fn write_tdr(&mut self, byte: u8) {
        self.events.push(Event::Write(byte));
    }

    fn read_if(&mut self) -> u32 {
        let value = self.if_values.pop_front().unwrap_or(self.default_if);
        self.events.push(Event::If(value));
        value
    }
}

fn limit() -> NonZeroU32 {
    NonZeroU32::new(2).unwrap()
}

#[test]
fn plaintext_reply_matches_literal_full_trace() {
    let mut payload = [0x15, 0x05, 0x04, 0x00, 0x11, 0x22, 0x33];
    let mut io = Mock {
        default_if: 0x80,
        ..Mock::default()
    };

    assert_eq!(send_reply(&mut io, &mut payload, false, limit()), Ok(()));
    assert_eq!(payload, [0x15, 0x05, 0x04, 0x00, 0x11, 0x22, 0x33]);
    assert_eq!(
        io.events,
        [
            Event::Boundary(SendPhase::Header),
            Event::Write(0xab),
            Event::If(0x80),
            Event::Write(0xcd),
            Event::If(0x80),
            Event::Write(0x07),
            Event::If(0x80),
            Event::Write(0x00),
            Event::If(0x80),
            Event::Boundary(SendPhase::Payload),
            Event::Write(0x15),
            Event::If(0x80),
            Event::Write(0x05),
            Event::If(0x80),
            Event::Write(0x04),
            Event::If(0x80),
            Event::Write(0x00),
            Event::If(0x80),
            Event::Write(0x11),
            Event::If(0x80),
            Event::Write(0x22),
            Event::If(0x80),
            Event::Write(0x33),
            Event::If(0x80),
            Event::Boundary(SendPhase::Footer),
            Event::Write(0xff),
            Event::If(0x80),
            Event::Write(0xff),
            Event::If(0x80),
            Event::Write(0xdc),
            Event::If(0x80),
            Event::Write(0xba),
            Event::If(0x80),
        ]
    );
}

#[test]
fn encrypted_reply_matches_literal_full_trace_and_mutates_inner_header() {
    let mut payload = [0x15, 0x05, 0x04, 0x00, 0x11, 0x22, 0x33];
    let mut io = Mock::default();

    assert_eq!(send_reply(&mut io, &mut payload, true, limit()), Ok(()));
    assert_eq!(payload, [0x03, 0x69, 0x10, 0xe6, 0x3f, 0xb3, 0x3e]);
    assert_eq!(
        io.first_reply_at_send.as_deref(),
        Some(&[0x03, 0x69, 0x10, 0xe6, 0x3f, 0xb3, 0x3e][..])
    );
    assert_eq!(
        io.events,
        [
            Event::Boundary(SendPhase::Header),
            Event::Write(0xab),
            Event::If(0),
            Event::Write(0xcd),
            Event::If(0),
            Event::Write(0x07),
            Event::If(0),
            Event::Write(0x00),
            Event::If(0),
            Event::Boundary(SendPhase::Payload),
            Event::Write(0x03),
            Event::If(0),
            Event::Write(0x69),
            Event::If(0),
            Event::Write(0x10),
            Event::If(0),
            Event::Write(0xe6),
            Event::If(0),
            Event::Write(0x3f),
            Event::If(0),
            Event::Write(0xb3),
            Event::If(0),
            Event::Write(0x3e),
            Event::If(0),
            Event::Boundary(SendPhase::Footer),
            Event::Write(0xbf),
            Event::If(0),
            Event::Write(0xde),
            Event::If(0),
            Event::Write(0xdc),
            Event::If(0),
            Event::Write(0xba),
            Event::If(0),
        ]
    );
}

#[test]
fn empty_payload_keeps_all_three_uart_send_boundaries() {
    let mut payload = [];
    let mut io = Mock::default();

    assert_eq!(send_reply(&mut io, &mut payload, false, limit()), Ok(()));
    assert_eq!(
        io.events,
        [
            Event::Boundary(SendPhase::Header),
            Event::Write(0xab),
            Event::If(0),
            Event::Write(0xcd),
            Event::If(0),
            Event::Write(0x00),
            Event::If(0),
            Event::Write(0x00),
            Event::If(0),
            Event::Boundary(SendPhase::Payload),
            Event::Boundary(SendPhase::Footer),
            Event::Write(0xff),
            Event::If(0),
            Event::Write(0xff),
            Event::If(0),
            Event::Write(0xdc),
            Event::If(0),
            Event::Write(0xba),
            Event::If(0),
        ]
    );
}

#[test]
fn polling_uses_fresh_if_reads_and_ignores_unrelated_flags() {
    let mut payload = [];
    let mut io = Mock {
        if_values: [UART_IF_TXFIFO_FULL_MASK | 0x20, 0x20].into(),
        default_if: 0x20,
        ..Mock::default()
    };

    assert_eq!(send_reply(&mut io, &mut payload, false, limit()), Ok(()));
    assert_eq!(
        &io.events[..6],
        [
            Event::Boundary(SendPhase::Header),
            Event::Write(0xab),
            Event::If(UART_IF_TXFIFO_FULL_MASK | 0x20),
            Event::If(0x20),
            Event::Write(0xcd),
            Event::If(0x20),
        ]
    );
}

#[test]
fn xor_keystream_wraps_and_footer_padding_uses_wrapped_length() {
    let mut payload = [0; 18];
    let mut io = Mock::default();

    assert_eq!(send_reply(&mut io, &mut payload, true, limit()), Ok(()));
    assert_eq!(
        payload,
        [
            0x16, 0x6c, 0x14, 0xe6, 0x2e, 0x91, 0x0d, 0x40, 0x21, 0x35, 0xd5, 0x40, 0x13, 0x03,
            0xe9, 0x80, 0x16, 0x6c
        ]
    );
    let writes: Vec<u8> = io
        .events
        .iter()
        .filter_map(|event| match event {
            Event::Write(byte) => Some(*byte),
            _ => None,
        })
        .collect();
    assert_eq!(
        writes,
        [
            0xab, 0xcd, 0x12, 0x00, 0x16, 0x6c, 0x14, 0xe6, 0x2e, 0x91, 0x0d, 0x40, 0x21, 0x35,
            0xd5, 0x40, 0x13, 0x03, 0xe9, 0x80, 0x16, 0x6c, 0xeb, 0x19, 0xdc, 0xba
        ]
    );
}

#[test]
fn oversized_payload_is_rejected_without_mutation_or_io() {
    let mut payload = vec![0x5a; usize::from(u16::MAX) + 1];
    let before = payload.clone();
    let mut io = Mock::default();

    assert_eq!(
        send_reply(&mut io, &mut payload, true, limit()),
        Err(ReplyError::PayloadTooLarge {
            len: usize::from(u16::MAX) + 1
        })
    );
    assert_eq!(payload, before);
    assert!(io.events.is_empty());
}

#[test]
fn maximum_u16_payload_sends_the_full_reply() {
    #[derive(Default)]
    struct Counter {
        phases: Vec<SendPhase>,
        writes: u32,
        reads: u32,
        first: Vec<u8>,
        last: Vec<u8>,
    }

    impl FifoBackend for Counter {
        fn begin_uart_send(&mut self, phase: SendPhase, _reply_payload: &[u8]) {
            self.phases.push(phase);
        }

        fn write_tdr(&mut self, byte: u8) {
            self.writes += 1;
            if self.first.len() < 4 {
                self.first.push(byte);
            }
            self.last.push(byte);
            if self.last.len() > 4 {
                self.last.remove(0);
            }
        }

        fn read_if(&mut self) -> u32 {
            self.reads += 1;
            0x80
        }
    }

    let mut payload = vec![0x5a; usize::from(u16::MAX)];
    let mut io = Counter::default();

    assert_eq!(send_reply(&mut io, &mut payload, false, limit()), Ok(()));
    assert_eq!(
        io.phases,
        [SendPhase::Header, SendPhase::Payload, SendPhase::Footer]
    );
    assert_eq!(io.writes, u32::from(u16::MAX) + 8);
    assert_eq!(io.reads, io.writes);
    assert_eq!(io.first, [0xab, 0xcd, 0xff, 0xff]);
    assert_eq!(io.last, [0xff, 0xff, 0xdc, 0xba]);
}

#[test]
fn timeout_in_each_section_reports_submitted_bytes_and_stops() {
    for (phase, successful_prefix, submitted) in [
        (SendPhase::Header, 0, 1),
        (SendPhase::Payload, 4, 5),
        (SendPhase::Footer, 7, 8),
    ] {
        let mut payload = [0x01, 0x02, 0x03];
        let mut io = Mock {
            if_values: vec![0; successful_prefix].into(),
            default_if: UART_IF_TXFIFO_FULL_MASK | 0x20,
            ..Mock::default()
        };
        assert_eq!(
            send_reply(&mut io, &mut payload, true, limit()),
            Err(ReplyError::FifoTimeout {
                phase,
                bytes_submitted: submitted
            })
        );
        assert_eq!(payload, [0x17, 0x6e, 0x17]);
        let boundaries: Vec<SendPhase> = io
            .events
            .iter()
            .filter_map(|event| match event {
                Event::Boundary(phase) => Some(*phase),
                _ => None,
            })
            .collect();
        let expected_boundaries = match phase {
            SendPhase::Header => vec![SendPhase::Header],
            SendPhase::Payload => vec![SendPhase::Header, SendPhase::Payload],
            SendPhase::Footer => {
                vec![SendPhase::Header, SendPhase::Payload, SendPhase::Footer]
            }
        };
        assert_eq!(boundaries, expected_boundaries);
        assert_eq!(
            io.events
                .iter()
                .filter(|event| matches!(event, Event::Write(_)))
                .count(),
            submitted as usize
        );
        assert_eq!(
            &io.events[io.events.len() - 2..],
            [
                Event::If(UART_IF_TXFIFO_FULL_MASK | 0x20),
                Event::If(UART_IF_TXFIFO_FULL_MASK | 0x20)
            ]
        );
    }
}

#[test]
fn target_register_addresses_match_the_pinned_uart_layout() {
    assert_eq!(UART1_BASE, 0x4006_b800);
    assert_eq!(UART1_TDR, 0x4006_b808);
    assert_eq!(UART1_IF, 0x4006_b814);
    assert_eq!(UART_IF_TXFIFO_FULL_MASK, 0x4000);
}
