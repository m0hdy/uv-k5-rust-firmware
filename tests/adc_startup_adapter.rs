#[path = "../src/adc_startup_adapter.rs"]
mod adc_startup_adapter;

use adc_startup_adapter::physical::{WordIo, apply_with_words};
use adc_startup_adapter::{Interrupt, IrqAction, Register, RegisterIo, apply_with_io};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Read(Register, u32),
    Write(Register, u32),
    Irq(Interrupt, IrqAction),
}

struct FakeRegisterIo {
    values: [u32; 9],
    scripted_reads: [VecDeque<u32>; 9],
    events: Vec<Event>,
}

impl FakeRegisterIo {
    fn new(values: [u32; 9], scripted_reads: [VecDeque<u32>; 9]) -> Self {
        Self {
            values,
            scripted_reads,
            events: Vec::new(),
        }
    }

    fn index(register: Register) -> usize {
        match register {
            Register::DeviceClockGate => 0,
            Register::ClockSelector => 1,
            Register::Configuration => 2,
            Register::ExternalTriggerSelector => 3,
            Register::CalibrationOffset => 4,
            Register::CalibrationKd => 5,
            Register::InterruptFlags => 6,
            Register::InterruptEnable => 7,
            Register::Start => 8,
        }
    }
}

impl RegisterIo for FakeRegisterIo {
    fn read(&mut self, register: Register) -> u32 {
        let index = Self::index(register);
        let value = self.scripted_reads[index]
            .pop_front()
            .unwrap_or(self.values[index]);
        self.events.push(Event::Read(register, value));
        value
    }

    fn write(&mut self, register: Register, value: u32) {
        self.events.push(Event::Write(register, value));
        self.values[Self::index(register)] = value;
    }

    fn set_irq(&mut self, interrupt: Interrupt, action: IrqAction) {
        self.events.push(Event::Irq(interrupt, action));
    }
}

fn empty_reads() -> [VecDeque<u32>; 9] {
    core::array::from_fn(|_| VecDeque::new())
}

#[test]
fn address_free_register_interface_records_the_full_order_and_fresh_reads() {
    let mut reads = empty_reads();
    reads[0] = VecDeque::from([0xa5a5_51aa]);
    reads[1] = VecDeque::from([0xa5a5_5aaa]);
    reads[2] = VecDeque::from([0xafff_ffff, 0x1357_9bff, 0xd6ad_0000]);
    reads[4] = VecDeque::from([0xa5a4_5aaa]);
    reads[5] = VecDeque::from([0x1356_9bdf]);
    reads[7] = VecDeque::from([0xdead_beef, 0]);
    reads[8] = VecDeque::from([0xa5a5_5aaf, 0x2468_1ff3]);

    let mut io = FakeRegisterIo::new([0; 9], reads);
    apply_with_io(&mut io, 0xa55a);

    assert_eq!(
        io.events,
        vec![
            Event::Irq(Interrupt::Saradc, IrqAction::Disable),
            Event::Read(Register::DeviceClockGate, 0xa5a5_51aa),
            Event::Write(Register::DeviceClockGate, 0xa7a5_51aa),
            Event::Read(Register::Configuration, 0xafff_ffff),
            Event::Write(Register::Configuration, 0xa7ff_ffff),
            Event::Read(Register::ClockSelector, 0xa5a5_5aaa),
            Event::Write(Register::ClockSelector, 0xa5a5_56aa),
            Event::Read(Register::Configuration, 0x1357_9bff),
            Event::Write(Register::Configuration, 0x07c3_8210),
            Event::Write(Register::ExternalTriggerSelector, 0xa55a),
            Event::Read(Register::CalibrationOffset, 0xa5a4_5aaa),
            Event::Write(Register::CalibrationOffset, 0xa5a5_5aaa),
            Event::Read(Register::CalibrationKd, 0x1356_9bdf),
            Event::Write(Register::CalibrationKd, 0x1357_9bdf),
            Event::Write(Register::InterruptFlags, u32::MAX),
            Event::Read(Register::InterruptEnable, 0xdead_beef),
            Event::Write(Register::InterruptEnable, 0xdeac_0000),
            Event::Read(Register::Configuration, 0xd6ad_0000),
            Event::Write(Register::Configuration, 0xdead_0000),
            Event::Read(Register::Start, 0xa5a5_5aaf),
            Event::Write(Register::Start, 0xa5a5_5aab),
            Event::Read(Register::Start, 0x2468_1ff3),
            Event::Write(Register::Start, 0x2468_1ff7),
        ]
    );
}

#[test]
fn stale_interrupt_enable_state_never_enables_saradc_irq() {
    let mut reads = empty_reads();
    reads[7] = VecDeque::from([0x8003_ffff]);

    let mut io = FakeRegisterIo::new([0; 9], reads);
    apply_with_io(&mut io, 0x1234);

    assert_eq!(
        io.events.first(),
        Some(&Event::Irq(Interrupt::Saradc, IrqAction::Disable))
    );
    assert!(
        io.events
            .iter()
            .filter(|event| matches!(event, Event::Irq(_, _)))
            .all(|event| matches!(event, Event::Irq(_, IrqAction::Disable)))
    );
    assert!(
        io.events
            .contains(&Event::Write(Register::InterruptEnable, 0x8000_0000))
    );
}

#[derive(Debug, Eq, PartialEq)]
enum PhysicalEvent {
    Read32(usize, u32),
    Write32(usize, u32),
    SyncIrqDisable,
}

struct FakeWordIo {
    registers: BTreeMap<usize, u32>,
    enabled_irqs: u32,
    events: Vec<PhysicalEvent>,
}

impl WordIo for FakeWordIo {
    fn read_word(&mut self, address: usize) -> u32 {
        assert_eq!(address % 4, 0);
        let value = self.registers[&address];
        self.events.push(PhysicalEvent::Read32(address, value));
        value
    }

    fn write_word(&mut self, address: usize, value: u32) {
        assert_eq!(address % 4, 0);
        self.events.push(PhysicalEvent::Write32(address, value));
        match address {
            0xe000_e100 => self.enabled_irqs |= value,
            0xe000_e180 => self.enabled_irqs &= !value,
            0x400b_a00c => *self.registers.get_mut(&address).unwrap() &= !value,
            _ => *self.registers.get_mut(&address).unwrap() = value,
        }
    }

    fn sync_irq_disable(&mut self) {
        self.events.push(PhysicalEvent::SyncIrqDisable);
    }
}

#[test]
fn physical_startup_uses_word_transactions_and_only_changes_the_saradc_nvic_line() {
    use PhysicalEvent::{Read32, SyncIrqDisable, Write32};

    for (preserved_ie, trigger, initial_nvic, final_nvic) in
        [(0, 0xa55a, 0x38, 0x28), (0x8000_0000, 0xfedc, 0x28, 0x28)]
    {
        let initial_ie = preserved_ie | 0x0003_ffff;
        let mut io = FakeWordIo {
            registers: BTreeMap::from([
                (0x4000_0008, 0x4000_0040),
                (0x4000_0000, 0xa5a5_5a2a),
                (0x400b_a000, 0xf800_8000),
                (0x400b_a0b0, 0xdead_beef),
                (0x400b_a0f0, 0xa5a4_5aaa),
                (0x400b_a0f4, 0x1356_9bdf),
                (0x400b_a00c, 0x0003_0210),
                (0x400b_a008, initial_ie),
                (0x400b_a004, 0x8000_0014),
            ]),
            enabled_irqs: initial_nvic,
            events: Vec::new(),
        };
        apply_with_words(&mut io, trigger);

        let mut expected = vec![
            Write32(0xe000_e180, 0x10),
            SyncIrqDisable,
            Read32(0x4000_0008, 0x4000_0040),
            Write32(0x4000_0008, 0x4200_0040),
            Read32(0x400b_a000, 0xf800_8000),
            Write32(0x400b_a000, 0xf000_8000),
            Read32(0x4000_0000, 0xa5a5_5a2a),
            Write32(0x4000_0000, 0xa5a5_56aa),
            Read32(0x400b_a000, 0xf000_8000),
            Write32(0x400b_a000, 0xc7c3_8210),
            Write32(0x400b_a0b0, u32::from(trigger)),
            Read32(0x400b_a0f0, 0xa5a4_5aaa),
            Write32(0x400b_a0f0, 0xa5a5_5aaa),
            Read32(0x400b_a0f4, 0x1356_9bdf),
            Write32(0x400b_a0f4, 0x1357_9bdf),
            Write32(0x400b_a00c, u32::MAX),
            Read32(0x400b_a008, initial_ie),
            Write32(0x400b_a008, preserved_ie),
        ];
        expected.extend([
            Read32(0x400b_a000, 0xc7c3_8210),
            Write32(0x400b_a000, 0xcfc3_8210),
            Read32(0x400b_a004, 0x8000_0014),
            Write32(0x400b_a004, 0x8000_0010),
            Read32(0x400b_a004, 0x8000_0010),
            Write32(0x400b_a004, 0x8000_0014),
        ]);
        assert_eq!(io.events, expected, "trigger {trigger:#06x}");
        assert_eq!(io.registers[&0x400b_a008], preserved_ie);
        assert_eq!(io.registers[&0x400b_a00c], 0);
        assert_eq!(io.registers[&0x400b_a0b0], u32::from(trigger));
        assert_eq!(io.enabled_irqs, final_nvic);
    }
}
