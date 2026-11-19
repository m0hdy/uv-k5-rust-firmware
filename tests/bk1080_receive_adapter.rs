pub use uv_k5_rust_firmware::{eeprom_i2c_bridge, eeprom_read_transport, i2c_bitbang_flow};

#[path = "../src/bk1080_receive_adapter.rs"]
mod bk1080_receive_adapter;

use bk1080_receive_adapter::{
    DelayMs, FrequencyDeviation, ReceiveError, Register, read_frequency_deviation, read_register,
    set_frequency, set_muted, write_register,
};
use eeprom_read_transport::{Acknowledgement, I2cTransport};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Start,
    Write(u8),
    Read(usize),
    Stop,
    DelayMs(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BusError {
    Transfer(usize),
    Cleanup,
}

type Trace = Rc<RefCell<Vec<Event>>>;

struct Bus {
    trace: Trace,
    read_data: VecDeque<u8>,
    fail_at: Option<usize>,
    nack_at: Option<usize>,
    fail_stop: bool,
}

impl Bus {
    fn new(trace: Trace, read_data: impl IntoIterator<Item = u8>) -> Self {
        Self {
            trace,
            read_data: read_data.into_iter().collect(),
            fail_at: None,
            nack_at: None,
            fail_stop: false,
        }
    }

    fn record(&mut self, event: Event) -> Result<usize, BusError> {
        let mut trace = self.trace.borrow_mut();
        let index = trace.len();
        trace.push(event);
        if self.fail_at == Some(index) {
            Err(BusError::Transfer(index))
        } else if event == Event::Stop && self.fail_stop {
            Err(BusError::Cleanup)
        } else {
            Ok(index)
        }
    }
}

impl I2cTransport for Bus {
    type Error = BusError;

    fn start(&mut self) -> Result<(), BusError> {
        self.record(Event::Start).map(|_| ())
    }

    fn repeated_start(&mut self) -> Result<(), BusError> {
        panic!("BK1080 register transactions do not use repeated START")
    }

    fn write_byte(&mut self, byte: u8) -> Result<Acknowledgement, BusError> {
        let index = self.record(Event::Write(byte))?;
        Ok(if self.nack_at == Some(index) {
            Acknowledgement::Nack
        } else {
            Acknowledgement::Ack
        })
    }

    fn read_buffer(&mut self, output: &mut [u8]) -> Result<(), BusError> {
        let result = self.record(Event::Read(output.len()));
        let count = if result.is_err() { 1 } else { output.len() };
        for byte in output.iter_mut().take(count) {
            *byte = self.read_data.pop_front().expect("scripted read byte");
        }
        result.map(|_| ())
    }

    fn stop(&mut self) -> Result<(), BusError> {
        self.record(Event::Stop).map(|_| ())
    }
}

struct Delay(Trace);

impl DelayMs for Delay {
    fn delay_ms(&mut self, milliseconds: u16) {
        self.0.borrow_mut().push(Event::DelayMs(milliseconds));
    }
}

fn bus_with_data(data: &[u8]) -> (Bus, Trace) {
    let trace = Trace::default();
    (Bus::new(trace.clone(), data.iter().copied()), trace)
}

#[test]
fn register_write_and_read_have_exact_big_endian_frames_without_repeated_start() {
    let (mut bus, trace) = bus_with_data(&[0x12, 0x34]);
    write_register(&mut bus, Register::SystemConfiguration2, 0xabcd).unwrap();
    assert_eq!(
        *trace.borrow(),
        [
            Event::Start,
            Event::Write(0x80),
            Event::Write(0x0a),
            Event::Write(0xab),
            Event::Write(0xcd),
            Event::Stop,
        ]
    );

    trace.borrow_mut().clear();
    assert_eq!(
        read_register(&mut bus, Register::SystemConfiguration2),
        Ok(0x1234)
    );
    assert_eq!(
        *trace.borrow(),
        [
            Event::Start,
            Event::Write(0x80),
            Event::Write(0x0b),
            Event::Read(2),
            Event::Stop,
        ]
    );
}

#[test]
fn complete_initialization_register_range_includes_decimal_25_internal_register() {
    assert_eq!(Register::from_index(0).map(Register::index), Some(0));
    assert_eq!(Register::from_index(33).map(Register::index), Some(33));
    assert_eq!(Register::from_index(34), None);
    assert_eq!(Register::Internal25.index(), 25);
    assert_eq!(Register::from_index(25), Some(Register::Internal25));

    let (mut bus, trace) = bus_with_data(&[]);
    write_register(&mut bus, Register::Internal25, 0xa83c).unwrap();
    assert_eq!(
        *trace.borrow(),
        [
            Event::Start,
            Event::Write(0x80),
            Event::Write(0x32),
            Event::Write(0xa8),
            Event::Write(0x3c),
            Event::Stop,
        ]
    );
}

#[test]
fn tuning_reads_fresh_configuration_and_executes_plan_in_order_with_ten_ms_delay() {
    let (mut bus, trace) = bus_with_data(&[0xa5, 0x5a]);
    let mut delay = Delay(trace.clone());

    assert_eq!(set_frequency(&mut bus, &mut delay, 900, 2), Ok(()));
    assert_eq!(
        *trace.borrow(),
        [
            Event::Start,
            Event::Write(0x80),
            Event::Write(0x0b),
            Event::Read(2),
            Event::Stop,
            Event::Start,
            Event::Write(0x80),
            Event::Write(0x0a),
            Event::Write(0xa5),
            Event::Write(0x9a),
            Event::Stop,
            Event::Start,
            Event::Write(0x80),
            Event::Write(0x06),
            Event::Write(0x00),
            Event::Write(0x8c),
            Event::Stop,
            Event::DelayMs(10),
            Event::Start,
            Event::Write(0x80),
            Event::Write(0x06),
            Event::Write(0x80),
            Event::Write(0x8c),
            Event::Stop,
        ]
    );
}

#[test]
fn tuning_preserves_unrelated_bits_and_wraps_channel_for_modulo_selected_band() {
    let (mut bus, trace) = bus_with_data(&[0x12, 0x34]);
    let mut delay = Delay(trace.clone());

    assert_eq!(set_frequency(&mut bus, &mut delay, 639, 7), Ok(()));
    let writes: Vec<u8> = trace
        .borrow()
        .iter()
        .filter_map(|event| match event {
            Event::Write(byte) => Some(*byte),
            _ => None,
        })
        .collect();
    assert_eq!(
        writes,
        [
            0x80, 0x0b, 0x80, 0x0a, 0x12, 0xf4, 0x80, 0x06, 0xff, 0xff, 0x80, 0x06, 0xff, 0xff,
        ]
    );
    assert_eq!(
        trace
            .borrow()
            .iter()
            .filter(|event| **event == Event::DelayMs(10))
            .count(),
        1
    );
}

#[test]
fn mute_and_deviation_execute_the_typed_register_operations() {
    for (muted, expected) in [(true, [0x42, 0x01]), (false, [0x02, 0x01])] {
        let (mut bus, trace) = bus_with_data(&[]);
        assert_eq!(set_muted(&mut bus, muted), Ok(()));
        assert_eq!(
            *trace.borrow(),
            [
                Event::Start,
                Event::Write(0x80),
                Event::Write(0x04),
                Event::Write(expected[0]),
                Event::Write(expected[1]),
                Event::Stop,
            ]
        );
    }

    let (mut bus, trace) = bus_with_data(&[0xab, 0xcd]);
    assert_eq!(
        read_frequency_deviation(&mut bus, 995),
        Ok(FrequencyDeviation {
            base_frequency: 995,
            deviation: 0x0abc,
        })
    );
    assert_eq!(
        *trace.borrow(),
        [
            Event::Start,
            Event::Write(0x80),
            Event::Write(0x0f),
            Event::Read(2),
            Event::Stop,
        ]
    );
}

#[test]
fn write_failures_nacks_and_final_stop_follow_single_cleanup_policy() {
    for fail_at in 0..6 {
        let (mut bus, trace) = bus_with_data(&[]);
        bus.fail_at = Some(fail_at);
        bus.fail_stop = true;
        assert_eq!(
            write_register(&mut bus, Register::Channel, 0x1234),
            Err(ReceiveError::Transport(BusError::Transfer(fail_at)))
        );
        let mut expected = [
            Event::Start,
            Event::Write(0x80),
            Event::Write(0x06),
            Event::Write(0x12),
            Event::Write(0x34),
            Event::Stop,
        ][..=fail_at]
            .to_vec();
        if (1..5).contains(&fail_at) {
            expected.push(Event::Stop);
        }
        assert_eq!(*trace.borrow(), expected, "failure at operation {fail_at}");
    }

    for nack_at in 1..5 {
        let (mut bus, trace) = bus_with_data(&[]);
        bus.nack_at = Some(nack_at);
        bus.fail_stop = true;
        assert_eq!(
            write_register(&mut bus, Register::Channel, 0x1234),
            Err(ReceiveError::NotAcknowledged)
        );
        let mut expected = [
            Event::Start,
            Event::Write(0x80),
            Event::Write(0x06),
            Event::Write(0x12),
            Event::Write(0x34),
            Event::Stop,
        ][..=nack_at]
            .to_vec();
        expected.push(Event::Stop);
        assert_eq!(*trace.borrow(), expected, "NACK at operation {nack_at}");
    }

    let (mut bus, trace) = bus_with_data(&[]);
    bus.fail_stop = true;
    assert_eq!(
        write_register(&mut bus, Register::Channel, 0x1234),
        Err(ReceiveError::Transport(BusError::Cleanup))
    );
    assert_eq!(
        trace
            .borrow()
            .iter()
            .filter(|event| **event == Event::Stop)
            .count(),
        1
    );
}

#[test]
fn read_failures_and_nacks_clean_up_without_returning_partial_or_default_values() {
    for fail_at in 0..5 {
        let (mut bus, trace) = bus_with_data(&[0x12, 0x34]);
        bus.fail_at = Some(fail_at);
        bus.fail_stop = true;
        assert_eq!(
            read_register(&mut bus, Register::Status),
            Err(ReceiveError::Transport(BusError::Transfer(fail_at)))
        );
        let mut expected = [
            Event::Start,
            Event::Write(0x80),
            Event::Write(0x0f),
            Event::Read(2),
            Event::Stop,
        ][..=fail_at]
            .to_vec();
        if (1..4).contains(&fail_at) {
            expected.push(Event::Stop);
        }
        assert_eq!(*trace.borrow(), expected, "failure at operation {fail_at}");
    }

    for nack_at in [1, 2] {
        let (mut bus, trace) = bus_with_data(&[0x12, 0x34]);
        bus.nack_at = Some(nack_at);
        assert_eq!(
            read_register(&mut bus, Register::Status),
            Err(ReceiveError::NotAcknowledged)
        );
        assert_eq!(
            *trace.borrow(),
            [
                Event::Start,
                Event::Write(0x80),
                Event::Write(0x0f),
                Event::Stop,
            ][..=nack_at]
                .iter()
                .copied()
                .chain([Event::Stop])
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn tuning_stops_at_first_write_error_without_delay_or_later_channel_write() {
    let (mut bus, trace) = bus_with_data(&[0xa5, 0x5a]);
    bus.fail_at = Some(13);
    let mut delay = Delay(trace.clone());

    assert_eq!(
        set_frequency(&mut bus, &mut delay, 900, 2),
        Err(ReceiveError::Transport(BusError::Transfer(13)))
    );
    assert_eq!(
        *trace.borrow(),
        [
            Event::Start,
            Event::Write(0x80),
            Event::Write(0x0b),
            Event::Read(2),
            Event::Stop,
            Event::Start,
            Event::Write(0x80),
            Event::Write(0x0a),
            Event::Write(0xa5),
            Event::Write(0x9a),
            Event::Stop,
            Event::Start,
            Event::Write(0x80),
            Event::Write(0x06),
            Event::Stop,
        ]
    );
    assert!(!trace.borrow().contains(&Event::DelayMs(10)));
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PinEvent {
    Scl(i2c_bitbang_flow::Level),
    Sda(i2c_bitbang_flow::Level),
    Read(i2c_bitbang_flow::Level),
}

struct Pins {
    samples: VecDeque<i2c_bitbang_flow::Level>,
    events: Vec<PinEvent>,
}

impl i2c_bitbang_flow::I2cIo for Pins {
    fn scl(&mut self, level: i2c_bitbang_flow::Level) {
        self.events.push(PinEvent::Scl(level));
    }

    fn sda(&mut self, level: i2c_bitbang_flow::Level) {
        self.events.push(PinEvent::Sda(level));
    }

    fn sda_input_enable(&mut self, _: bool) {}
    fn sda_open_drain(&mut self, _: bool) {}
    fn sda_direction(&mut self, _: i2c_bitbang_flow::Direction) {}

    fn read_sda(&mut self) -> i2c_bitbang_flow::Level {
        let level = self.samples.pop_front().expect("scripted SDA sample");
        self.events.push(PinEvent::Read(level));
        level
    }

    fn delay_1us(&mut self) {}
}

#[test]
fn register_read_through_real_bitbang_bridge_acks_then_nacks_payload_bytes() {
    use i2c_bitbang_flow::Level::{High, Low};

    let mut samples = VecDeque::from([Low, Low]);
    for byte in [0x12, 0x34] {
        samples.extend(
            (0..8)
                .rev()
                .map(|bit| if byte & (1 << bit) == 0 { Low } else { High }),
        );
    }
    let mut pins = Pins {
        samples,
        events: Vec::new(),
    };
    let value = read_frequency_deviation(
        &mut eeprom_i2c_bridge::I2cBitbangTransport::new(&mut pins),
        875,
    )
    .unwrap();
    assert_eq!(
        value,
        FrequencyDeviation {
            base_frequency: 875,
            deviation: 0x0123,
        }
    );

    let mut samples_since_sda = 0;
    let mut acknowledgements = Vec::new();
    for event in &pins.events {
        match event {
            PinEvent::Read(_) => samples_since_sda += 1,
            PinEvent::Sda(level) => {
                if samples_since_sda == 8 {
                    acknowledgements.push(*level);
                }
                samples_since_sda = 0;
            }
            PinEvent::Scl(_) => {}
        }
    }
    assert_eq!(acknowledgements, [Low, High]);
    assert!(pins.samples.is_empty());
}
