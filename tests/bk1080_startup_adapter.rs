pub use uv_k5_rust_firmware::{eeprom_i2c_bridge, eeprom_read_transport, i2c_bitbang_flow};

#[path = "../src/bk1080_startup_adapter.rs"]
mod bk1080_startup_adapter;

use bk1080_startup_adapter::{ShutdownError, WordIo, shutdown_with_io};
use eeprom_read_transport::{Acknowledgement, I2cTransport};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Start,
    Write(u8),
    Stop,
    Scl(i2c_bitbang_flow::Level),
    Sda(i2c_bitbang_flow::Level),
    Delay,
    ReadWord(usize),
    WriteWord(usize, u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BusError {
    Transfer(usize),
    Cleanup,
}

type Trace = Rc<RefCell<Vec<Event>>>;

struct Bus {
    trace: Trace,
    fail_at: Option<usize>,
    nack_at: Option<usize>,
    fail_stop: bool,
}

impl Bus {
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
        panic!("unexpected repeated START")
    }

    fn write_byte(&mut self, byte: u8) -> Result<Acknowledgement, BusError> {
        let index = self.record(Event::Write(byte))?;
        Ok(if self.nack_at == Some(index) {
            Acknowledgement::Nack
        } else {
            Acknowledgement::Ack
        })
    }

    fn read_buffer(&mut self, _: &mut [u8]) -> Result<(), BusError> {
        panic!("unexpected data read")
    }

    fn stop(&mut self) -> Result<(), BusError> {
        self.record(Event::Stop).map(|_| ())
    }
}

struct Gpio {
    trace: Trace,
    data: u32,
}

impl WordIo for Gpio {
    fn read_word(&mut self, address: usize) -> u32 {
        self.trace.borrow_mut().push(Event::ReadWord(address));
        self.data
    }

    fn write_word(&mut self, address: usize, value: u32) {
        self.trace
            .borrow_mut()
            .push(Event::WriteWord(address, value));
        self.data = value;
    }
}

const BUS_TRACE: [Event; 6] = [
    Event::Start,
    Event::Write(0x80),
    Event::Write(0x04),
    Event::Write(0x02),
    Event::Write(0x41),
    Event::Stop,
];
const GPIO_TRACE: [Event; 2] = [
    Event::ReadWord(0x4006_0800),
    Event::WriteWord(0x4006_0800, 0xa5a5_d234),
];

fn fixtures() -> (Bus, Gpio, Trace) {
    let trace = Rc::new(RefCell::new(Vec::new()));
    (
        Bus {
            trace: trace.clone(),
            fail_at: None,
            nack_at: None,
            fail_stop: false,
        },
        Gpio {
            trace: trace.clone(),
            data: 0xa5a5_5234,
        },
        trace,
    )
}

#[test]
fn success_stops_before_fresh_gpio_read_and_preserves_other_bits() {
    let (mut bus, mut gpio, trace) = fixtures();
    assert_eq!(shutdown_with_io(&mut bus, &mut gpio), Ok(()));
    assert_eq!(
        *trace.borrow(),
        [BUS_TRACE.as_slice(), &GPIO_TRACE].concat()
    );
    assert_eq!(gpio.data, 0xa5a5_d234);
}

#[test]
fn every_transport_failure_has_expected_cleanup_and_error_precedence() {
    for fail_at in 0..BUS_TRACE.len() {
        for fail_stop in [false, true] {
            let (mut bus, mut gpio, trace) = fixtures();
            bus.fail_at = Some(fail_at);
            bus.fail_stop = fail_stop;
            assert_eq!(
                shutdown_with_io(&mut bus, &mut gpio),
                Err(ShutdownError::Transport(BusError::Transfer(fail_at)))
            );
            let mut expected = BUS_TRACE[..=fail_at].to_vec();
            if (1..5).contains(&fail_at) {
                expected.push(Event::Stop);
            }
            expected.extend(GPIO_TRACE);
            assert_eq!(*trace.borrow(), expected, "failure at {fail_at}");
        }
    }
}

#[test]
fn every_nack_stops_once_then_asserts_pb15_even_if_cleanup_fails() {
    for nack_at in 1..5 {
        for fail_stop in [false, true] {
            let (mut bus, mut gpio, trace) = fixtures();
            bus.nack_at = Some(nack_at);
            bus.fail_stop = fail_stop;
            assert_eq!(
                shutdown_with_io(&mut bus, &mut gpio),
                Err(ShutdownError::NotAcknowledged)
            );
            let mut expected = BUS_TRACE[..=nack_at].to_vec();
            expected.push(Event::Stop);
            expected.extend(GPIO_TRACE);
            assert_eq!(*trace.borrow(), expected, "NACK at {nack_at}");
        }
    }
}

struct Pins {
    trace: Trace,
    samples: usize,
    nack: bool,
}

impl i2c_bitbang_flow::I2cIo for Pins {
    fn scl(&mut self, level: i2c_bitbang_flow::Level) {
        self.trace.borrow_mut().push(Event::Scl(level));
    }

    fn sda(&mut self, level: i2c_bitbang_flow::Level) {
        self.trace.borrow_mut().push(Event::Sda(level));
    }

    fn sda_input_enable(&mut self, _: bool) {}
    fn sda_open_drain(&mut self, _: bool) {}
    fn sda_direction(&mut self, _: i2c_bitbang_flow::Direction) {}

    fn read_sda(&mut self) -> i2c_bitbang_flow::Level {
        self.samples += 1;
        if self.nack {
            i2c_bitbang_flow::Level::High
        } else {
            i2c_bitbang_flow::Level::Low
        }
    }

    fn delay_1us(&mut self) {
        self.trace.borrow_mut().push(Event::Delay);
    }
}

#[test]
fn bitbang_bridge_stops_before_gpio_on_ack_and_nack() {
    use i2c_bitbang_flow::Level::{High, Low};

    for nack in [false, true] {
        let (_, mut gpio, trace) = fixtures();
        let mut pins = Pins {
            trace: trace.clone(),
            samples: 0,
            nack,
        };
        let result = shutdown_with_io(
            &mut eeprom_i2c_bridge::I2cBitbangTransport::new(&mut pins),
            &mut gpio,
        );
        assert_eq!(
            result,
            if nack {
                Err(ShutdownError::NotAcknowledged)
            } else {
                Ok(())
            }
        );
        assert_eq!(pins.samples, if nack { 255 } else { 4 });
        assert!(trace.borrow().ends_with(&[
            Event::Sda(Low),
            Event::Delay,
            Event::Scl(Low),
            Event::Delay,
            Event::Scl(High),
            Event::Delay,
            Event::Sda(High),
            Event::Delay,
            GPIO_TRACE[0],
            GPIO_TRACE[1],
        ]));
    }
}
