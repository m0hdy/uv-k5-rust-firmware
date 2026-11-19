use std::collections::VecDeque;
use uv_k5_rust_firmware::i2c_bitbang_flow::{
    Acknowledgement, Direction, I2cIo, Level, read_buffer, read_byte, start, stop, write_byte,
};

use Direction::{Input, Output};
use Level::{High, Low};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Op {
    Scl(Level),
    Sda(Level),
    InputEnable(bool),
    OpenDrain(bool),
    Direction(Direction),
    Read(Level),
    Delay,
}

#[derive(Default)]
struct Mock {
    trace: Vec<Op>,
    samples: VecDeque<Level>,
}

impl Mock {
    fn with_samples(samples: impl IntoIterator<Item = Level>) -> Self {
        Self {
            trace: Vec::new(),
            samples: samples.into_iter().collect(),
        }
    }

    fn assert_trace(&self, expected: &[Op]) {
        assert_eq!(self.trace, expected);
        assert!(
            self.samples.is_empty(),
            "not all supplied samples were read"
        );
    }
}

impl I2cIo for Mock {
    fn scl(&mut self, level: Level) {
        self.trace.push(Op::Scl(level));
    }

    fn sda(&mut self, level: Level) {
        self.trace.push(Op::Sda(level));
    }

    fn sda_input_enable(&mut self, enabled: bool) {
        self.trace.push(Op::InputEnable(enabled));
    }

    fn sda_open_drain(&mut self, enabled: bool) {
        self.trace.push(Op::OpenDrain(enabled));
    }

    fn sda_direction(&mut self, direction: Direction) {
        self.trace.push(Op::Direction(direction));
    }

    fn read_sda(&mut self) -> Level {
        let level = self.samples.pop_front().expect("unexpected SDA poll");
        self.trace.push(Op::Read(level));
        level
    }

    fn delay_1us(&mut self) {
        self.trace.push(Op::Delay);
    }
}

const INPUT: [Op; 3] = [
    Op::InputEnable(true),
    Op::OpenDrain(false),
    Op::Direction(Input),
];
const OUTPUT: [Op; 3] = [
    Op::InputEnable(false),
    Op::OpenDrain(true),
    Op::Direction(Output),
];

#[test]
fn start_and_stop_have_literal_edge_and_delay_order() {
    let mut io = Mock::default();
    start(&mut io);
    stop(&mut io);
    io.assert_trace(&[
        Op::Sda(High),
        Op::Delay,
        Op::Scl(High),
        Op::Delay,
        Op::Sda(Low),
        Op::Delay,
        Op::Scl(Low),
        Op::Delay,
        Op::Sda(Low),
        Op::Delay,
        Op::Scl(Low),
        Op::Delay,
        Op::Scl(High),
        Op::Delay,
        Op::Sda(High),
        Op::Delay,
    ]);
}

fn write_trace(bits: [Level; 8], samples: &[Level]) -> Vec<Op> {
    let mut trace = vec![Op::Scl(Low), Op::Delay];
    for bit in bits {
        trace.extend([
            Op::Sda(bit),
            Op::Delay,
            Op::Scl(High),
            Op::Delay,
            Op::Scl(Low),
            Op::Delay,
        ]);
    }
    trace.extend(INPUT);
    trace.extend([Op::Sda(High), Op::Delay, Op::Scl(High), Op::Delay]);
    trace.extend(samples.iter().copied().map(Op::Read));
    trace.extend([Op::Scl(Low), Op::Delay]);
    trace.extend(OUTPUT);
    trace.push(Op::Sda(High));
    trace
}

#[test]
fn write_mixed_bytes_msb_first_and_stop_polling_on_early_ack() {
    for (byte, bits, samples) in [
        (
            0xa5,
            [High, Low, High, Low, Low, High, Low, High],
            vec![Low],
        ),
        (
            0x36,
            [Low, Low, High, High, Low, High, High, Low],
            vec![High, High, Low],
        ),
    ] {
        let mut io = Mock::with_samples(samples.iter().copied());
        assert_eq!(write_byte(&mut io, byte), Acknowledgement::Ack);
        io.assert_trace(&write_trace(bits, &samples));
    }
}

#[test]
fn write_ack_on_sample_255_and_timeout_both_restore_pa11() {
    let bits = [High, Low, High, Low, Low, High, Low, High];
    for acknowledged in [true, false] {
        let mut samples = vec![High; 255];
        if acknowledged {
            samples[254] = Low;
        }
        let mut io = Mock::with_samples(samples.iter().copied());
        assert_eq!(
            write_byte(&mut io, 0xa5),
            if acknowledged {
                Acknowledgement::Ack
            } else {
                Acknowledgement::Nack
            }
        );
        io.assert_trace(&write_trace(bits, &samples));
    }
}

fn read_trace(samples: [Level; 8], response: Level) -> Vec<Op> {
    let mut trace = INPUT.to_vec();
    for sample in samples {
        trace.extend([
            Op::Scl(Low),
            Op::Delay,
            Op::Scl(High),
            Op::Delay,
            Op::Delay,
            Op::Read(sample),
            Op::Scl(Low),
            Op::Delay,
        ]);
    }
    trace.extend(OUTPUT);
    trace.extend([
        Op::Scl(Low),
        Op::Delay,
        Op::Sda(response),
        Op::Delay,
        Op::Scl(High),
        Op::Delay,
        Op::Scl(Low),
        Op::Delay,
    ]);
    trace
}

#[test]
fn read_mixed_bits_msb_first_and_transmit_ack_or_final_nack() {
    for (samples, byte, final_byte, response) in [
        (
            [High, Low, High, Low, Low, High, Low, High],
            0xa5,
            false,
            Low,
        ),
        (
            [Low, Low, High, High, Low, High, High, Low],
            0x36,
            true,
            High,
        ),
    ] {
        let mut io = Mock::with_samples(samples);
        assert_eq!(read_byte(&mut io, final_byte), byte);
        io.assert_trace(&read_trace(samples, response));
    }
}

#[test]
fn buffer_reads_delay_before_each_byte_and_nack_only_the_last() {
    let first = [High, Low, High, Low, Low, High, Low, High];
    let last = [Low, Low, High, High, Low, High, High, Low];
    let mut io = Mock::with_samples(first.into_iter().chain(last));
    let mut bytes = [0; 2];
    read_buffer(&mut io, &mut bytes);
    assert_eq!(bytes, [0xa5, 0x36]);
    let mut expected = vec![Op::Delay];
    expected.extend(read_trace(first, Low));
    expected.push(Op::Delay);
    expected.extend(read_trace(last, High));
    io.assert_trace(&expected);

    let mut io = Mock::default();
    read_buffer(&mut io, &mut []);
    io.assert_trace(&[]);

    let mut io = Mock::with_samples(first);
    let mut byte = [0];
    read_buffer(&mut io, &mut byte);
    assert_eq!(byte, [0xa5]);
    let mut expected = vec![Op::Delay];
    expected.extend(read_trace(first, High));
    io.assert_trace(&expected);
}
