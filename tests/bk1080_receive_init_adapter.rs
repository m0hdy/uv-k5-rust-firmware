pub use uv_k5_rust_firmware::{eeprom_i2c_bridge, eeprom_read_transport, i2c_bitbang_flow};

#[path = "../src/bk1080_receive_init_adapter.rs"]
mod bk1080_receive_init_adapter;

use bk1080_receive_init_adapter::{
    DelayMs, InitializationError, Initialized, ReceiveError, WordIo, initialize_with_io,
};
use eeprom_read_transport::{Acknowledgement, I2cTransport};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

const GPIOB_DATA: usize = 0x4006_0800;
const PB15: u32 = 1 << 15;
const INITIALIZATION_TABLE: [u16; 34] = [
    0x0008, 0x1080, 0x0201, 0x0000, 0x40c0, 0x0a1f, 0x002e, 0x02ff, 0x5b11, 0x0000, 0x411e, 0x0000,
    0xce00, 0x0000, 0x0000, 0x1000, 0x3197, 0x0000, 0x13ff, 0x9852, 0x0000, 0x0000, 0x0008, 0x0000,
    0x51e1, 0xa8bc, 0x2645, 0x00e4, 0x1cd8, 0x3a50, 0xeae0, 0x3000, 0x0200, 0x0000,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Start,
    Write(u8),
    Read(usize),
    Stop,
    DelayMs(u16),
    GpioRead(usize),
    GpioWrite(usize, u32),
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

struct Gpio {
    trace: Trace,
    data: u32,
}

impl WordIo for Gpio {
    fn read_word(&mut self, address: usize) -> u32 {
        self.trace.borrow_mut().push(Event::GpioRead(address));
        self.data
    }

    fn write_word(&mut self, address: usize, value: u32) {
        self.trace
            .borrow_mut()
            .push(Event::GpioWrite(address, value));
        self.data = value;
    }
}

fn write_register(events: &mut Vec<Event>, index: u8, value: u16) {
    events.extend([
        Event::Start,
        Event::Write(0x80),
        Event::Write(index << 1),
        Event::Write((value >> 8) as u8),
        Event::Write(value as u8),
        Event::Stop,
    ]);
}

fn read_register(events: &mut Vec<Event>, index: u8) {
    events.extend([
        Event::Start,
        Event::Write(0x80),
        Event::Write((index << 1) | 1),
        Event::Read(2),
        Event::Stop,
    ]);
}

fn expected_tuning(events: &mut Vec<Event>) {
    read_register(events, 5);
    write_register(events, 5, 0xa59a);
    write_register(events, 3, 0x008c);
    events.push(Event::DelayMs(10));
    write_register(events, 3, 0x808c);
}

#[test]
fn cold_initialization_has_exact_table_calibration_timing_and_tune_trace() {
    let trace = Trace::default();
    let gpio_initial = 0xa5a5_8001;
    let mut gpio = Gpio {
        trace: trace.clone(),
        data: gpio_initial,
    };
    let mut bus = Bus::new(trace.clone(), [0xa5, 0x5a]);
    let mut delay = Delay(trace.clone());
    let mut initialized = Initialized::new();

    assert_eq!(
        initialize_with_io(&mut bus, &mut gpio, &mut delay, 900, 2, &mut initialized),
        Ok(())
    );
    assert!(initialized.is_initialized());
    assert_eq!(gpio.data, gpio_initial & !PB15);

    let mut expected = vec![
        Event::GpioRead(GPIOB_DATA),
        Event::GpioWrite(GPIOB_DATA, gpio_initial & !PB15),
    ];
    for (index, value) in INITIALIZATION_TABLE.iter().copied().enumerate() {
        write_register(&mut expected, index as u8, value);
    }
    expected.push(Event::DelayMs(250));
    write_register(&mut expected, 25, 0xa83c);
    write_register(&mut expected, 25, 0xa8bc);
    expected.push(Event::DelayMs(60));
    write_register(&mut expected, 5, 0x0a1f);
    expected_tuning(&mut expected);

    assert_eq!(*trace.borrow(), expected);
}

#[test]
fn warm_enable_skips_calibration_and_uses_the_same_tune_executor() {
    let trace = Trace::default();
    let gpio_initial = 0x9123_8001;
    let mut gpio = Gpio {
        trace: trace.clone(),
        data: gpio_initial,
    };
    let mut bus = Bus::new(trace.clone(), [0xa5, 0x5a]);
    let mut delay = Delay(trace.clone());
    let mut initialized = Initialized::new();

    assert_eq!(
        initialize_with_io(&mut bus, &mut gpio, &mut delay, 900, 2, &mut initialized),
        Ok(())
    );
    assert!(initialized.is_initialized());
    trace.borrow_mut().clear();
    bus.read_data.extend([0xa5, 0x5a]);
    let warm_gpio_data = gpio.data;
    assert_eq!(
        initialize_with_io(&mut bus, &mut gpio, &mut delay, 900, 2, &mut initialized),
        Ok(())
    );
    assert!(initialized.is_initialized());
    assert_eq!(gpio.data, warm_gpio_data & !PB15);

    let mut expected = vec![
        Event::GpioRead(GPIOB_DATA),
        Event::GpioWrite(GPIOB_DATA, warm_gpio_data & !PB15),
    ];
    write_register(&mut expected, 2, 0x0201);
    write_register(&mut expected, 5, 0x0a1f);
    expected_tuning(&mut expected);
    assert_eq!(*trace.borrow(), expected);
}

#[test]
fn zero_frequency_is_rejected_without_gpio_bus_or_state_changes() {
    let trace = Trace::default();
    let gpio_initial = 0x8123_8001;
    let mut gpio = Gpio {
        trace: trace.clone(),
        data: gpio_initial,
    };
    let mut bus = Bus::new(trace.clone(), []);
    let mut delay = Delay(trace.clone());
    let mut initialized = Initialized::new();

    assert_eq!(
        initialize_with_io(&mut bus, &mut gpio, &mut delay, 0, 2, &mut initialized),
        Err(InitializationError::ZeroFrequency)
    );
    assert!(!initialized.is_initialized());
    assert_eq!(gpio.data, gpio_initial);
    assert!(trace.borrow().is_empty());
}

#[test]
fn early_nack_cleans_up_before_reasserting_pb15_and_leaves_state_incomplete() {
    let trace = Trace::default();
    let gpio_initial = 0x4001_8005;
    let mut gpio = Gpio {
        trace: trace.clone(),
        data: gpio_initial,
    };
    let mut bus = Bus::new(trace.clone(), []);
    bus.nack_at = Some(4);
    let mut delay = Delay(trace.clone());
    let mut initialized = Initialized::new();

    assert_eq!(
        initialize_with_io(&mut bus, &mut gpio, &mut delay, 900, 2, &mut initialized),
        Err(InitializationError::Receive(ReceiveError::NotAcknowledged))
    );
    assert!(!initialized.is_initialized());
    assert_eq!(gpio.data, gpio_initial | PB15);
    assert_eq!(
        *trace.borrow(),
        [
            Event::GpioRead(GPIOB_DATA),
            Event::GpioWrite(GPIOB_DATA, gpio_initial & !PB15),
            Event::Start,
            Event::Write(0x80),
            Event::Write(0x00),
            Event::Stop,
            Event::GpioRead(GPIOB_DATA),
            Event::GpioWrite(GPIOB_DATA, gpio_initial | PB15),
        ]
    );
}

#[test]
fn tuning_failure_after_calibration_retains_state_and_reasserts_pb15_after_cleanup() {
    let trace = Trace::default();
    let gpio_initial = 0x1234_8009;
    let mut gpio = Gpio {
        trace: trace.clone(),
        data: gpio_initial,
    };
    let mut bus = Bus::new(trace.clone(), [0xa5, 0x5a]);
    let failing_read = 2 + INITIALIZATION_TABLE.len() * 6 + 1 + 2 * 6 + 1 + 6 + 3;
    bus.fail_at = Some(failing_read);
    let mut delay = Delay(trace.clone());
    let mut initialized = Initialized::new();

    assert_eq!(
        initialize_with_io(&mut bus, &mut gpio, &mut delay, 900, 2, &mut initialized),
        Err(InitializationError::Receive(ReceiveError::Transport(
            BusError::Transfer(failing_read)
        )))
    );
    assert!(initialized.is_initialized());
    assert_eq!(gpio.data, gpio_initial | PB15);

    let events = trace.borrow();
    assert_eq!(
        &events[events.len() - 13..],
        [
            Event::Start,
            Event::Write(0x80),
            Event::Write(0x0a),
            Event::Write(0x0a),
            Event::Write(0x1f),
            Event::Stop,
            Event::Start,
            Event::Write(0x80),
            Event::Write(0x0b),
            Event::Read(2),
            Event::Stop,
            Event::GpioRead(GPIOB_DATA),
            Event::GpioWrite(GPIOB_DATA, gpio_initial | PB15),
        ]
    );
    assert!(events.contains(&Event::DelayMs(250)));
    assert!(events.contains(&Event::DelayMs(60)));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::DelayMs(250 | 60)))
            .count(),
        2
    );
}
