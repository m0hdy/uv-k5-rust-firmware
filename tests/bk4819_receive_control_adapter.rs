#[path = "../src/bk4819_receive_control_adapter.rs"]
mod adapter;

use adapter::{Delay1us, Register, WordIo};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Event {
    Read(Register, u32),
    Write(Register, u32),
    Delay,
}

#[derive(Default)]
struct Trace {
    events: Vec<Event>,
    gpio_data: u32,
    decoded: Vec<(u8, u16)>,
    selected: bool,
    bits: Vec<bool>,
}

impl Trace {
    fn record_write(&mut self, value: u32) {
        let previous = self.gpio_data;
        self.gpio_data = value;
        let scn_was_high = previous & 1 != 0;
        let scn_is_high = value & 1 != 0;
        let scl_was_high = previous & 2 != 0;
        let scl_is_high = value & 2 != 0;

        if scn_was_high && !scn_is_high {
            assert!(!self.selected);
            self.selected = true;
            self.bits.clear();
        }

        if self.selected && !scl_was_high && scl_is_high {
            self.bits.push(value & 4 != 0);
        }

        if self.selected && !scn_was_high && scn_is_high {
            assert!(self.selected);
            assert_eq!(self.bits.len(), 24);
            let address = self.bits[..8]
                .iter()
                .fold(0u8, |byte, bit| (byte << 1) | u8::from(*bit));
            let value = self.bits[8..]
                .iter()
                .fold(0u16, |word, bit| (word << 1) | u16::from(*bit));
            self.decoded.push((address, value));
            self.selected = false;
        }
    }

    fn assert_transactions(&self, expected: &[(u8, u16)]) {
        assert_eq!(self.decoded, expected);
        assert!(!self.selected);
        assert_eq!(self.events.len(), expected.len() * 236);
        assert_eq!(
            self.events
                .iter()
                .filter(|event| matches!(event, Event::Read(Register::GpioCData, _)))
                .count(),
            expected.len() * 80
        );
        assert_eq!(
            self.events
                .iter()
                .filter(|event| matches!(event, Event::Write(Register::GpioCData, _)))
                .count(),
            expected.len() * 80
        );
        assert_eq!(
            self.events
                .iter()
                .filter(|event| matches!(event, Event::Delay))
                .count(),
            expected.len() * 76
        );

        let mut events = self.events.iter();
        while let Some(event) = events.next() {
            match event {
                Event::Read(Register::GpioCData, read_value) => {
                    let Some(Event::Write(Register::GpioCData, written)) = events.next() else {
                        panic!("each GPIO DATA read must be followed by its write");
                    };
                    assert_eq!((read_value ^ written) & !7, 0);
                }
                Event::Write(_, _) => panic!("write without its preceding GPIO DATA read"),
                Event::Delay => {}
                Event::Read(register, _) => {
                    panic!("unexpected register read: {register:?}");
                }
            }
        }
    }
}

struct TraceIo(Rc<RefCell<Trace>>);

struct TraceDelay(Rc<RefCell<Trace>>);

impl WordIo for TraceIo {
    fn read_word(&mut self, register: Register) -> u32 {
        let mut trace = self.0.borrow_mut();
        let value = trace.gpio_data;
        trace.events.push(Event::Read(register, value));
        value
    }

    fn write_word(&mut self, register: Register, value: u32) {
        let mut trace = self.0.borrow_mut();
        trace.events.push(Event::Write(register, value));
        trace.record_write(value);
    }
}

impl Delay1us for TraceDelay {
    fn delay_1us(&mut self) {
        self.0.borrow_mut().events.push(Event::Delay);
    }
}

#[test]
fn receive_on_emits_the_complete_gpio_transactions_in_order() {
    let trace = Rc::new(RefCell::new(Trace::default()));
    let mut io = TraceIo(trace.clone());
    let mut delay = TraceDelay(trace.clone());
    adapter::receive_on(&mut io, &mut delay);
    trace
        .borrow()
        .assert_transactions(&[(0x37, 0x1F0F), (0x30, 0x0000), (0x30, 0xBFF1)]);
}

#[test]
fn sleep_emits_only_the_complete_gpio_transactions_in_order() {
    let trace = Rc::new(RefCell::new(Trace::default()));
    let mut io = TraceIo(trace.clone());
    let mut delay = TraceDelay(trace.clone());
    adapter::sleep(&mut io, &mut delay);
    trace
        .borrow()
        .assert_transactions(&[(0x30, 0x0000), (0x37, 0x1D00)]);
}

#[test]
fn idle_emits_only_its_complete_gpio_transaction() {
    let trace = Rc::new(RefCell::new(Trace::default()));
    let mut io = TraceIo(trace.clone());
    let mut delay = TraceDelay(trace.clone());
    adapter::idle(&mut io, &mut delay);
    trace.borrow().assert_transactions(&[(0x30, 0x0000)]);
}

#[test]
fn receive_on_register_30_value_uses_only_the_pinned_receive_enable_fields() {
    const VCO_CALIB: u16 = 1 << 15;
    const UNKNOWN: u16 = 1 << 14;
    const RX_LINK: u16 = 0xF << 10;
    const AF_DAC: u16 = 1 << 9;
    const DISC_MODE: u16 = 1 << 8;
    const PLL_VCO: u16 = 0xF << 4;
    const PA_GAIN: u16 = 1 << 3;
    const MIC_ADC: u16 = 1 << 2;
    const TX_DSP: u16 = 1 << 1;
    const RX_DSP: u16 = 1;

    let expected = VCO_CALIB | RX_LINK | AF_DAC | DISC_MODE | PLL_VCO | RX_DSP;
    assert_eq!(expected, 0xBFF1);
    assert_eq!(expected & (UNKNOWN | PA_GAIN | MIC_ADC | TX_DSP), 0);
}
