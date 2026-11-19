use std::collections::VecDeque;
use uv_k5_rust_firmware::eeprom_i2c_bridge::{
    Features, ReadError, SettingsBootBlock, SettingsBootReadError, read_settings_boot,
};
use uv_k5_rust_firmware::i2c_bitbang_flow::target_gpio::{
    Delay1us, GpioI2cIo, Register, RegisterIo, SCL_MASK, SDA_MASK,
};
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RegisterEvent {
    Read(Register, u32),
    Write(Register, u32),
}

impl RegisterEvent {
    fn register(self) -> Register {
        match self {
            Self::Read(register, _) | Self::Write(register, _) => register,
        }
    }
}

struct RegisterState {
    values: [u32; 4],
    events: Vec<RegisterEvent>,
    samples: VecDeque<Level>,
    last_input: Level,
    pending_sample: bool,
    delays: usize,
}

type SharedRegisters = std::rc::Rc<std::cell::RefCell<RegisterState>>;

impl RegisterState {
    fn new(samples: impl IntoIterator<Item = Level>) -> Self {
        Self {
            values: [
                0x1234_5678,
                0x89ab_cdef | SDA_MASK,
                0x1357_9bdf & !SDA_MASK,
                0x2468_ace0 & !SDA_MASK,
            ],
            events: Vec::new(),
            samples: samples.into_iter().collect(),
            last_input: High,
            pending_sample: false,
            delays: 0,
        }
    }

    fn index(register: Register) -> usize {
        match register {
            Register::GpioAData => 0,
            Register::GpioADirection => 1,
            Register::PortAInputEnable => 2,
            Register::PortAOpenDrain => 3,
        }
    }

    fn finish_sample_read(&mut self) {
        if self.pending_sample {
            if let Some(level) = self.samples.pop_front() {
                self.last_input = level;
            }
            self.pending_sample = false;
        }
    }

    fn read(&mut self, register: Register) -> u32 {
        self.finish_sample_read();
        let index = Self::index(register);
        let mut value = self.values[index];
        if register == Register::GpioAData && self.values[1] & SDA_MASK == 0 {
            let level = self.samples.front().copied().unwrap_or(self.last_input);
            value = match level {
                Low => value & !SDA_MASK,
                High => value | SDA_MASK,
            };
            self.pending_sample = true;
        }
        self.events.push(RegisterEvent::Read(register, value));
        value
    }

    fn write(&mut self, register: Register, value: u32) {
        self.pending_sample = false;
        self.values[Self::index(register)] = value;
        self.events.push(RegisterEvent::Write(register, value));
    }
}

struct MockRegisters(SharedRegisters);

impl RegisterIo for MockRegisters {
    fn read(&mut self, register: Register) -> u32 {
        self.0.borrow_mut().read(register)
    }

    fn write(&mut self, register: Register, value: u32) {
        self.0.borrow_mut().write(register, value);
    }
}

struct MockDelay(SharedRegisters);

impl Delay1us for MockDelay {
    fn delay_1us(&mut self) {
        self.0.borrow_mut().delays += 1;
    }
}

fn target_mock(
    samples: impl IntoIterator<Item = Level>,
) -> (GpioI2cIo<MockRegisters, MockDelay>, SharedRegisters) {
    let shared = std::rc::Rc::new(std::cell::RefCell::new(RegisterState::new(samples)));
    (
        GpioI2cIo::new(MockRegisters(shared.clone()), MockDelay(shared.clone())),
        shared,
    )
}

fn assert_register_rmw_preserves_unrelated_bits(events: &[RegisterEvent]) {
    let mut last_reads = [None; 4];
    for event in events {
        match *event {
            RegisterEvent::Read(register, value) => {
                last_reads[RegisterState::index(register)] = Some(value);
            }
            RegisterEvent::Write(register, value) => {
                let index = RegisterState::index(register);
                let old = last_reads[index].expect("write must use a fresh register read");
                let affected = match register {
                    Register::GpioAData => SCL_MASK | SDA_MASK,
                    Register::GpioADirection
                    | Register::PortAInputEnable
                    | Register::PortAOpenDrain => SDA_MASK,
                };
                assert_eq!(
                    (old ^ value) & !affected,
                    0,
                    "unrelated bits changed in {register:?}"
                );
                last_reads[index] = None;
            }
        }
    }
}

#[test]
fn gpio_backend_uses_fresh_register_reads_and_samples_sda() {
    let (mut io, shared) = target_mock([Low]);
    io.scl(High);
    io.sda(Low);
    io.sda_input_enable(true);
    io.sda_open_drain(true);
    io.sda_direction(Input);
    assert_eq!(io.read_sda(), Low);
    io.delay_1us();
    io.handoff_to_keypad();

    let state = shared.borrow();
    assert_eq!(
        [
            Register::GpioAData.address(),
            Register::GpioADirection.address(),
            Register::PortAInputEnable.address(),
            Register::PortAOpenDrain.address(),
        ],
        [0x4006_0000, 0x4006_0004, 0x400b_0100, 0x400b_0400]
    );
    assert_eq!(
        state
            .events
            .iter()
            .map(|event| event.register())
            .collect::<Vec<_>>(),
        [
            Register::GpioAData,
            Register::GpioAData,
            Register::GpioAData,
            Register::GpioAData,
            Register::PortAInputEnable,
            Register::PortAInputEnable,
            Register::PortAOpenDrain,
            Register::PortAOpenDrain,
            Register::GpioADirection,
            Register::GpioADirection,
            Register::GpioAData,
            Register::GpioAData,
            Register::GpioAData,
            Register::PortAInputEnable,
            Register::PortAInputEnable,
            Register::PortAOpenDrain,
            Register::PortAOpenDrain,
            Register::GpioADirection,
            Register::GpioADirection,
        ]
    );
    assert!(matches!(
        state.events[10],
        RegisterEvent::Read(Register::GpioAData, value) if value & SDA_MASK == 0
    ));
    assert_eq!(state.delays, 1);
    assert!(state.samples.is_empty());
    assert_register_rmw_preserves_unrelated_bits(&state.events);
    assert_eq!(state.values[0] & (SCL_MASK | SDA_MASK), SCL_MASK | SDA_MASK);
    assert_eq!(state.values[1] & SDA_MASK, SDA_MASK);
    assert_eq!(state.values[2] & SDA_MASK, 0);
    assert_eq!(state.values[3] & SDA_MASK, 0);
}

#[test]
fn gpio_backend_returns_sampled_ack_and_nack_and_restores_sda_mode() {
    for (sample, expected) in [(Low, Acknowledgement::Ack), (High, Acknowledgement::Nack)] {
        let (mut io, shared) = target_mock([sample]);
        assert_eq!(write_byte(&mut io, 0xa5), expected);
        {
            let state = shared.borrow();
            assert_eq!(state.delays, 28);
            assert!(state.samples.is_empty());
            assert_eq!(state.values[1] & SDA_MASK, SDA_MASK);
            assert_eq!(state.values[2] & SDA_MASK, 0);
            assert_eq!(state.values[3] & SDA_MASK, SDA_MASK);
            assert_register_rmw_preserves_unrelated_bits(&state.events);
        }
        io.handoff_to_keypad();
        let state = shared.borrow();
        assert_eq!(state.values[1] & SDA_MASK, SDA_MASK);
        assert_eq!(state.values[2] & SDA_MASK, 0);
        assert_eq!(state.values[3] & SDA_MASK, 0);
    }
}

const STARTUP_FEATURES: Features = Features {
    noaa_enabled: true,
    vox_enabled: true,
    blmin_tmp_off_enabled: true,
};

fn startup_samples(responses: &[[u8; 8]; 2]) -> Vec<Level> {
    let mut samples = Vec::new();
    for response in responses {
        samples.extend([Low; 4]);
        for byte in response {
            for bit in (0..8).rev() {
                samples.push(if byte & (1 << bit) == 0 { Low } else { High });
            }
        }
    }
    samples
}

#[test]
fn gpio_backend_reads_startup_settings_through_the_canonical_bridge() {
    let responses = [[73, 8, 10, 1, 1, 1, 9, 4], [0x24, 3, 1, 4, 2, 5, 1, 1]];
    let (mut io, shared) = target_mock(startup_samples(&responses));

    let settings = read_settings_boot(&mut io, STARTUP_FEATURES).unwrap();
    assert_eq!(settings.core.call_channel, 73);
    assert_eq!(settings.core.vox_level, Some(9));
    assert_eq!(settings.display.channel_display_mode, 3);
    assert_eq!(settings.display.backlight_time, 5);
    {
        let state = shared.borrow();
        assert_eq!(state.delays, 840);
        assert!(state.samples.is_empty());
        assert_eq!(state.values[0] & (SCL_MASK | SDA_MASK), SCL_MASK | SDA_MASK);
        assert_register_rmw_preserves_unrelated_bits(&state.events);
    }
    io.handoff_to_keypad();
    let state = shared.borrow();
    assert_eq!(state.values[1] & SDA_MASK, SDA_MASK);
    assert_eq!(state.values[2] & SDA_MASK, 0);
    assert_eq!(state.values[3] & SDA_MASK, 0);
}

#[test]
fn gpio_backend_keeps_bridge_stop_cleanup_on_first_block_nack() {
    let (mut io, shared) = target_mock([High; 255]);

    assert_eq!(
        read_settings_boot(&mut io, STARTUP_FEATURES),
        Err(SettingsBootReadError {
            block: SettingsBootBlock::Core,
            error: ReadError::NotAcknowledged,
        })
    );
    {
        let state = shared.borrow();
        assert_eq!(state.delays, 36);
        assert!(state.samples.is_empty());
        assert_eq!(state.values[0] & (SCL_MASK | SDA_MASK), SCL_MASK | SDA_MASK);
        assert_eq!(state.values[1] & SDA_MASK, SDA_MASK);
        assert_eq!(state.values[2] & SDA_MASK, 0);
        assert_eq!(state.values[3] & SDA_MASK, SDA_MASK);
        let data_writes = state
            .events
            .iter()
            .filter_map(|event| match event {
                RegisterEvent::Write(Register::GpioAData, value) => {
                    Some(value & (SCL_MASK | SDA_MASK))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(data_writes.ends_with(&[0, 0, SCL_MASK, SCL_MASK | SDA_MASK]));
        assert_register_rmw_preserves_unrelated_bits(&state.events);
    }
    io.handoff_to_keypad();
    let state = shared.borrow();
    assert_eq!(state.values[1] & SDA_MASK, SDA_MASK);
    assert_eq!(state.values[2] & SDA_MASK, 0);
    assert_eq!(state.values[3] & SDA_MASK, 0);
}
