#[path = "../src/bk4819_startup_adapter.rs"]
mod startup;

use startup::transport::{Delay1us, Register, WordIo};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Transaction {
    Read(u8, u16),
    Write(u8, u16),
}

struct Bus {
    data: u32,
    direction: u32,
    input_enable: u32,
    agc_value: u16,
    read_bit: usize,
    transaction_active: bool,
    first_transaction_gpio_event_count: Option<usize>,
    address_bits: Vec<bool>,
    value_bits: Vec<bool>,
    read_transaction: bool,
    transactions: Vec<Transaction>,
    initial_gpio_events: Vec<(bool, u32)>,
}

impl Bus {
    fn new(agc_value: u16) -> Self {
        Self {
            data: 0xa5a5_1200,
            direction: startup::transport::SDA_MASK,
            input_enable: 0xfeed_0000,
            agc_value,
            read_bit: 0,
            transaction_active: false,
            first_transaction_gpio_event_count: None,
            address_bits: Vec::new(),
            value_bits: Vec::new(),
            read_transaction: false,
            transactions: Vec::new(),
            initial_gpio_events: Vec::new(),
        }
    }

    fn current_word(&self, register: Register) -> u32 {
        match register {
            Register::GpioCData => self.data,
            Register::GpioCDirection => self.direction,
            Register::PortCInputEnable => self.input_enable,
        }
    }

    fn finish_transaction(&mut self) {
        let address = bits_to_u16(&self.address_bits) as u8;
        if self.read_transaction {
            self.transactions
                .push(Transaction::Read(address & 0x7f, self.agc_value));
        } else {
            self.transactions
                .push(Transaction::Write(address, bits_to_u16(&self.value_bits)));
        }
        self.transaction_active = false;
        self.address_bits.clear();
        self.value_bits.clear();
        self.read_transaction = false;
        self.read_bit = 0;
    }
}

fn bits_to_u16(bits: &[bool]) -> u16 {
    bits.iter()
        .fold(0, |value, bit| (value << 1) | u16::from(*bit))
}

impl WordIo for Bus {
    fn read_word(&mut self, register: Register) -> u32 {
        let mut value = self.current_word(register);
        if register == Register::GpioCData
            && self.direction & startup::transport::SDA_MASK == 0
            && self.transaction_active
        {
            let bit = self.read_bit < 16 && self.agc_value & (1 << (15 - self.read_bit)) != 0;
            value = (value & !startup::transport::SDA_MASK)
                | if bit { startup::transport::SDA_MASK } else { 0 };
        }
        value
    }

    fn write_word(&mut self, register: Register, value: u32) {
        match register {
            Register::GpioCData => {
                if self.initial_gpio_events.len() < 6 {
                    self.initial_gpio_events.push((false, self.data));
                    self.initial_gpio_events.push((true, value));
                }

                let old = self.data;
                self.data = value;
                if old & startup::transport::SCN_MASK != 0
                    && value & startup::transport::SCN_MASK == 0
                {
                    if self.first_transaction_gpio_event_count.is_none() {
                        self.first_transaction_gpio_event_count =
                            Some(self.initial_gpio_events.len());
                    }
                    self.transaction_active = true;
                    self.address_bits.clear();
                    self.value_bits.clear();
                    self.read_transaction = false;
                    self.read_bit = 0;
                } else if old & startup::transport::SCN_MASK == 0
                    && value & startup::transport::SCN_MASK != 0
                    && self.transaction_active
                {
                    self.finish_transaction();
                }

                if self.transaction_active
                    && old & startup::transport::SCL_MASK == 0
                    && value & startup::transport::SCL_MASK != 0
                {
                    if self.direction & startup::transport::SDA_MASK == 0 {
                        self.read_bit += 1;
                    } else if self.address_bits.len() < 8 {
                        self.address_bits
                            .push(value & startup::transport::SDA_MASK != 0);
                        if self.address_bits.len() == 8 {
                            let address = bits_to_u16(&self.address_bits) as u8;
                            self.read_transaction = address & 0x80 != 0;
                            if self.read_transaction {
                                assert_eq!(address, 0xfe);
                            }
                        }
                    } else if !self.read_transaction {
                        self.value_bits
                            .push(value & startup::transport::SDA_MASK != 0);
                    }
                }
            }
            Register::GpioCDirection => self.direction = value,
            Register::PortCInputEnable => self.input_enable = value,
        }
    }
}

struct NoDelay;

impl Delay1us for NoDelay {
    fn delay_1us(&mut self) {}
}

fn expected_transactions(agc_value: u16) -> Vec<Transaction> {
    let mut expected = vec![
        Transaction::Write(0x00, 0x8000),
        Transaction::Write(0x00, 0x0000),
        Transaction::Write(0x37, 0x1d0f),
        Transaction::Write(0x36, 0x0022),
        Transaction::Write(0x13, 0x03be),
        Transaction::Write(0x12, 0x037b),
        Transaction::Write(0x11, 0x027b),
        Transaction::Write(0x10, 0x007a),
        Transaction::Write(0x14, 0x0019),
        Transaction::Write(0x49, 0x2a38),
        Transaction::Write(0x7b, 0x8420),
        Transaction::Read(0x7e, agc_value),
    ];
    if agc_value & 0x8000 != 0 {
        expected.push(Transaction::Write(0x7e, 0x3000 | (agc_value & 0x0fff)));
    }
    expected.extend([
        Transaction::Write(0x19, 0x1041),
        Transaction::Write(0x7d, 0xe940),
        Transaction::Write(0x48, 0xb3a8),
        Transaction::Write(0x09, 0x006f),
        Transaction::Write(0x09, 0x106b),
        Transaction::Write(0x09, 0x2067),
        Transaction::Write(0x09, 0x3062),
        Transaction::Write(0x09, 0x4050),
        Transaction::Write(0x09, 0x5047),
        Transaction::Write(0x09, 0x603a),
        Transaction::Write(0x09, 0x702c),
        Transaction::Write(0x09, 0x8041),
        Transaction::Write(0x09, 0x9037),
        Transaction::Write(0x09, 0xa025),
        Transaction::Write(0x09, 0xb017),
        Transaction::Write(0x09, 0xc0e4),
        Transaction::Write(0x09, 0xd0cb),
        Transaction::Write(0x09, 0xe0b5),
        Transaction::Write(0x09, 0xf09f),
        Transaction::Write(0x1f, 0x5454),
        Transaction::Write(0x3e, 0xa037),
        Transaction::Write(0x33, 0x9000),
        Transaction::Write(0x3f, 0x0000),
    ]);
    expected
}

#[test]
fn cold_initialization_runs_complete_gpio_transport_for_both_agc_states() {
    for agc_value in [0x25bc, 0xa5bc] {
        let initial_data = 0xa5a5_1200;
        let mut bus = Bus::new(agc_value);
        let mut delay = NoDelay;
        let mut gpio_output_shadow = 0x4567;

        startup::initialize_with_io(&mut bus, &mut delay, &mut gpio_output_shadow);

        assert_eq!(
            &bus.initial_gpio_events[..6],
            &[
                (false, initial_data),
                (true, initial_data | 1),
                (false, initial_data | 1),
                (true, initial_data | 3),
                (false, initial_data | 3),
                (true, initial_data | 7),
            ]
        );
        assert_eq!(bus.first_transaction_gpio_event_count, Some(6));
        assert_eq!(bus.transactions, expected_transactions(agc_value));
        assert_eq!(
            bus.transactions.len(),
            if agc_value & 0x8000 == 0 { 35 } else { 36 }
        );
        assert_eq!(
            bus.transactions
                .iter()
                .filter(|transaction| matches!(transaction, Transaction::Write(_, _)))
                .count(),
            if agc_value & 0x8000 == 0 { 34 } else { 35 }
        );
        assert_eq!(gpio_output_shadow, 0x9000);
    }
}
