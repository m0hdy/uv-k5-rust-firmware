#[path = "../src/bk4819_register_transport.rs"]
mod transport;

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use transport::{Delay1us, Register, SDA_MASK, WordIo};

#[derive(Clone, Debug, Eq, PartialEq)]
enum Event {
    Read(Register, u32),
    Write(Register, u32),
    Delay,
}

struct Reference {
    events: Vec<Event>,
    reads: HashMap<Register, VecDeque<u32>>,
    read_counts: HashMap<Register, u32>,
    target_state: HashMap<Register, u32>,
    last_input_sda: bool,
}

impl Default for Reference {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            reads: HashMap::new(),
            read_counts: HashMap::new(),
            target_state: HashMap::from([(Register::GpioCDirection, SDA_MASK)]),
            last_input_sda: false,
        }
    }
}

impl Reference {
    fn snapshot(&mut self, register: Register) -> u32 {
        let count = self.read_counts.entry(register).or_default();
        let noise = 0xa55a_3cc3 ^ count.wrapping_mul(0x1020_4081);
        *count += 1;
        let target_mask = match register {
            Register::GpioCData => 7,
            Register::GpioCDirection | Register::PortCInputEnable => SDA_MASK,
        };
        let mut target_bits = *self.target_state.get(&register).unwrap_or(&0);
        if register == Register::GpioCData
            && self
                .target_state
                .get(&Register::GpioCDirection)
                .copied()
                .unwrap_or(0)
                & SDA_MASK
                == 0
        {
            target_bits =
                (target_bits & !SDA_MASK) | if self.last_input_sda { SDA_MASK } else { 0 };
        }
        (noise & !target_mask) | (target_bits & target_mask)
    }

    fn read_word(&mut self, register: Register, value: u32) {
        self.events.push(Event::Read(register, value));
        self.reads.entry(register).or_default().push_back(value);
    }

    fn update(&mut self, register: Register, mask: u32, high: bool) {
        let current = self.snapshot(register);
        self.read_word(register, current);
        let updated = if high {
            current | mask
        } else {
            current & !mask
        };
        self.events.push(Event::Write(register, updated));
        let target_mask = match register {
            Register::GpioCData => 7,
            Register::GpioCDirection | Register::PortCInputEnable => SDA_MASK,
        };
        self.target_state.insert(register, updated & target_mask);
    }

    fn scn(&mut self, high: bool) {
        self.update(Register::GpioCData, 1, high);
    }

    fn scl(&mut self, high: bool) {
        self.update(Register::GpioCData, 2, high);
    }

    fn sda(&mut self, high: bool) {
        self.update(Register::GpioCData, 4, high);
    }

    fn delay(&mut self) {
        self.events.push(Event::Delay);
    }

    fn write_address(&mut self, address: u8) {
        self.scl(false);
        for bit in (0..8).rev() {
            self.sda(address & (1 << bit) != 0);
            self.delay();
            self.scl(true);
            self.delay();
            self.scl(false);
            self.delay();
        }
    }

    fn write_value(&mut self, value: u16) {
        self.scl(false);
        for bit in (0..16).rev() {
            self.sda(value & (1 << bit) != 0);
            self.delay();
            self.scl(true);
            self.delay();
            self.scl(false);
            self.delay();
        }
    }

    fn read_value(&mut self, value: u16) {
        self.update(Register::PortCInputEnable, 4, true);
        self.update(Register::GpioCDirection, 4, false);
        self.delay();

        for bit in (0..16).rev() {
            self.last_input_sda = value & (1 << bit) != 0;
            let sample = self.snapshot(Register::GpioCData);
            self.read_word(Register::GpioCData, sample);
            self.scl(true);
            self.delay();
            self.scl(false);
            self.delay();
        }

        self.update(Register::PortCInputEnable, 4, false);
        self.update(Register::GpioCDirection, 4, true);
    }

    fn read_transaction(mut self, address: u8, value: u16) -> Self {
        self.scn(true);
        self.scl(false);
        self.delay();
        self.scn(false);
        self.write_address(address | 0x80);
        self.read_value(value);
        self.scn(true);
        self.delay();
        self.scl(true);
        self.sda(true);
        self
    }

    fn write_transaction(mut self, address: u8, value: u16) -> Self {
        self.scn(true);
        self.scl(false);
        self.delay();
        self.scn(false);
        self.write_address(address);
        self.delay();
        self.write_value(value);
        self.delay();
        self.scn(true);
        self.delay();
        self.scl(true);
        self.sda(true);
        self
    }
}

struct TraceIo {
    events: Rc<RefCell<Vec<Event>>>,
    reads: HashMap<Register, VecDeque<u32>>,
}

impl TraceIo {
    fn from_reference(reference: &Reference) -> Self {
        Self {
            events: Rc::new(RefCell::new(Vec::new())),
            reads: reference.reads.clone(),
        }
    }
}

struct TraceDelay(Rc<RefCell<Vec<Event>>>);

impl WordIo for TraceIo {
    fn read_word(&mut self, register: Register) -> u32 {
        let value = self
            .reads
            .get_mut(&register)
            .expect("reference supplies a snapshot for each read")
            .pop_front()
            .expect("reference snapshot queue is exhausted");
        self.events.borrow_mut().push(Event::Read(register, value));
        value
    }

    fn write_word(&mut self, register: Register, value: u32) {
        self.events.borrow_mut().push(Event::Write(register, value));
    }
}

impl Delay1us for TraceDelay {
    fn delay_1us(&mut self) {
        self.0.borrow_mut().push(Event::Delay);
    }
}

fn assert_rmw_preserves_unrelated_bits(events: &[Event]) {
    for (index, event) in events.iter().enumerate() {
        let Event::Write(register, written) = event else {
            continue;
        };
        let Event::Read(read_register, snapshot) = events[index - 1] else {
            panic!("each register write must use its immediately preceding fresh read");
        };
        assert_eq!(*register, read_register);
        let mask = match register {
            Register::GpioCData => 7,
            Register::GpioCDirection | Register::PortCInputEnable => SDA_MASK,
        };
        assert_eq!((written ^ snapshot) & !mask, 0);
    }
}

fn execute_read(address: u8, payload: u16) -> (Vec<Event>, u16) {
    let reference = Reference::default().read_transaction(address, payload);
    let mut io = TraceIo::from_reference(&reference);
    let mut delay = TraceDelay(io.events.clone());
    let result = transport::read_register(&mut io, &mut delay, address);
    let events = io.events.borrow().clone();
    assert_eq!(events, reference.events);
    assert!(io.reads.values().all(VecDeque::is_empty));
    assert_rmw_preserves_unrelated_bits(&events);
    assert_eq!(
        events
            .iter()
            .filter(|event| **event == Event::Delay)
            .count(),
        59
    );
    (events, result)
}

fn execute_write(address: u8, value: u16) -> Vec<Event> {
    let reference = Reference::default().write_transaction(address, value);
    let mut io = TraceIo::from_reference(&reference);
    let mut delay = TraceDelay(io.events.clone());
    transport::write_register(&mut io, &mut delay, address, value);
    let events = io.events.borrow().clone();
    assert_eq!(events, reference.events);
    assert!(io.reads.values().all(VecDeque::is_empty));
    assert_rmw_preserves_unrelated_bits(&events);
    assert_eq!(
        events
            .iter()
            .filter(|event| **event == Event::Delay)
            .count(),
        76
    );
    events
}

#[test]
fn read_matches_complete_gpio_turnaround_and_idle_trace() {
    for (address, payload) in [(0x26, 0xa53c), (0x00, 0x0000), (0xff, 0xffff)] {
        let (events, result) = execute_read(address, payload);
        assert_eq!(result, payload);

        let ie_enable = events
            .iter()
            .position(|event| matches!(event, Event::Write(Register::PortCInputEnable, value) if value & SDA_MASK != 0))
            .unwrap();
        let input_direction = events
            .iter()
            .position(|event| matches!(event, Event::Write(Register::GpioCDirection, value) if value & SDA_MASK == 0))
            .unwrap();
        let ie_disable = events
            .iter()
            .rposition(|event| matches!(event, Event::Write(Register::PortCInputEnable, value) if value & SDA_MASK == 0))
            .unwrap();
        let output_direction = events
            .iter()
            .rposition(|event| matches!(event, Event::Write(Register::GpioCDirection, value) if value & SDA_MASK != 0))
            .unwrap();
        assert!(ie_enable < input_direction);
        assert!(input_direction < ie_disable);
        assert!(ie_disable < output_direction);

        let final_data_writes: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                Event::Write(Register::GpioCData, value) => Some(value),
                _ => None,
            })
            .rev()
            .take(2)
            .collect();
        assert_eq!(final_data_writes.len(), 2);
        assert_eq!(final_data_writes[0] & 7, 7);
        assert_eq!(final_data_writes[1] & 3, 3);
    }
}

#[test]
fn write_matches_complete_gpio_trace_and_preserves_raw_address() {
    for (address, value) in [(0x35, 0x96a1), (0x00, 0x0000), (0xff, 0xffff)] {
        let events = execute_write(address, value);
        let data_writes: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                Event::Write(Register::GpioCData, value) => Some(*value),
                _ => None,
            })
            .collect();
        assert!(!data_writes.is_empty());
        assert_eq!(data_writes[data_writes.len() - 2] & 3, 3);
        assert_eq!(data_writes[data_writes.len() - 1] & 7, 7);
        assert_eq!(
            events
                .iter()
                .filter(|event| **event == Event::Delay)
                .count(),
            76
        );
    }
}

#[test]
fn register_addresses_map_to_pinned_dp32g030_words() {
    assert_eq!(Register::GpioCData.address(), 0x4006_1000);
    assert_eq!(Register::GpioCDirection.address(), 0x4006_1004);
    assert_eq!(Register::PortCInputEnable.address(), 0x400b_0108);
}
