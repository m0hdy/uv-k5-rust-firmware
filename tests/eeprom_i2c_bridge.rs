use std::collections::VecDeque;
use uv_k5_rust_firmware::eeprom_i2c_bridge::Level::{High, Low};
use uv_k5_rust_firmware::eeprom_i2c_bridge::{
    Acknowledgement, Direction, Features, I2cBitbangTransport, I2cIo, I2cTransport, Level,
    ReadError, SettingsBootBlock, SettingsBootReadError, read_settings_boot,
};

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
struct PinMock {
    trace: Vec<Op>,
    samples: VecDeque<Level>,
    read_count: usize,
}

impl PinMock {
    fn with_responses(responses: &[[u8; 8]; 2]) -> Self {
        let mut samples = VecDeque::new();
        for response in responses {
            samples.extend([Low; 4]);
            for byte in response {
                for bit in (0..8).rev() {
                    samples.push_back(if byte & (1 << bit) == 0 { Low } else { High });
                }
            }
        }
        Self {
            trace: Vec::new(),
            samples,
            read_count: 0,
        }
    }

    fn count_sequence(&self, sequence: &[Op]) -> usize {
        self.trace
            .windows(sequence.len())
            .filter(|window| *window == sequence)
            .count()
    }

    fn sequence_positions(&self, sequence: &[Op]) -> Vec<usize> {
        self.trace
            .windows(sequence.len())
            .enumerate()
            .filter_map(|(index, window)| (window == sequence).then_some(index))
            .collect()
    }

    fn read_byte_acknowledgements(&self) -> Vec<Level> {
        let mut samples_since_sda = 0;
        let mut acknowledgements = Vec::new();
        for event in &self.trace {
            match event {
                Op::Read(_) => samples_since_sda += 1,
                Op::Sda(level) => {
                    if samples_since_sda == 8 {
                        acknowledgements.push(*level);
                    }
                    samples_since_sda = 0;
                }
                _ => {}
            }
        }
        acknowledgements
    }
}

impl I2cIo for PinMock {
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
        self.read_count += 1;
        let level = self.samples.pop_front().expect("unexpected SDA sample");
        self.trace.push(Op::Read(level));
        level
    }

    fn delay_1us(&mut self) {
        self.trace.push(Op::Delay);
    }
}

const START: [Op; 8] = [
    Op::Sda(High),
    Op::Delay,
    Op::Scl(High),
    Op::Delay,
    Op::Sda(Low),
    Op::Delay,
    Op::Scl(Low),
    Op::Delay,
];

const STOP: [Op; 8] = [
    Op::Sda(Low),
    Op::Delay,
    Op::Scl(Low),
    Op::Delay,
    Op::Scl(High),
    Op::Delay,
    Op::Sda(High),
    Op::Delay,
];

const FEATURES: Features = Features {
    noaa_enabled: true,
    vox_enabled: true,
    blmin_tmp_off_enabled: true,
};

fn all_read_byte_acknowledgements() -> Vec<Level> {
    (0..2)
        .flat_map(|_| [Low, Low, Low, Low, Low, Low, Low, High])
        .collect()
}

#[test]
fn startup_reads_use_bitbang_start_repeated_start_ack_read_and_stop_flow() {
    let responses = [[73, 8, 10, 1, 1, 1, 9, 4], [0x24, 3, 1, 4, 2, 5, 1, 1]];
    let mut io = PinMock::with_responses(&responses);

    let settings = read_settings_boot(&mut io, FEATURES).unwrap();

    assert_eq!(settings.core.call_channel, 73);
    assert_eq!(settings.core.vox_level, Some(9));
    assert_eq!(settings.display.channel_display_mode, 3);
    assert_eq!(settings.display.backlight_time, 5);
    assert_eq!(io.count_sequence(&START), 4);
    assert_eq!(io.count_sequence(&STOP), 2);
    let starts = io.sequence_positions(&START);
    let stops = io.sequence_positions(&STOP);
    assert!(starts[0] < starts[1] && starts[1] < stops[0]);
    assert!(stops[0] < starts[2] && starts[2] < starts[3] && starts[3] < stops[1]);
    assert_eq!(io.read_count, 8 + 16 * 8);
    assert!(io.samples.is_empty());
    assert_eq!(
        io.read_byte_acknowledgements(),
        all_read_byte_acknowledgements()
    );
}

#[test]
fn erased_responses_are_read_as_data_in_either_startup_block() {
    let normal = [73, 8, 10, 1, 1, 1, 9, 4];
    let erased = [0xff; 8];
    let display = [0x24, 3, 1, 4, 2, 5, 1, 1];

    for responses in [[erased, display], [normal, erased]] {
        let mut io = PinMock::with_responses(&responses);
        assert!(read_settings_boot(&mut io, FEATURES).is_ok());
        assert_eq!(io.count_sequence(&START), 4);
        assert_eq!(io.count_sequence(&STOP), 2);
        assert_eq!(io.read_count, 8 + 16 * 8);
        assert_eq!(
            io.read_byte_acknowledgements(),
            all_read_byte_acknowledgements()
        );
        assert!(io.samples.is_empty());
    }
}

#[test]
fn transport_ack_conversion_uses_only_the_bitbang_ack_sample() {
    let mut io = PinMock {
        samples: [Low].into(),
        ..PinMock::default()
    };
    let mut transport = I2cBitbangTransport::new(&mut io);

    assert_eq!(
        I2cTransport::write_byte(&mut transport, 0xa0),
        Ok(Acknowledgement::Ack)
    );
    assert_eq!(io.read_count, 1);
    assert!(io.samples.is_empty());
}

#[test]
fn first_block_nack_stops_and_does_not_start_the_second_transaction() {
    let mut io = PinMock {
        samples: [High; 255].into(),
        ..PinMock::default()
    };

    assert_eq!(
        read_settings_boot(&mut io, FEATURES),
        Err(SettingsBootReadError {
            block: SettingsBootBlock::Core,
            error: ReadError::NotAcknowledged,
        })
    );
    assert_eq!(io.count_sequence(&START), 1);
    assert_eq!(io.count_sequence(&STOP), 1);
    assert_eq!(io.read_count, 255);
    assert!(io.samples.is_empty());
    assert!(io.read_byte_acknowledgements().is_empty());
}
