use std::collections::VecDeque;
use std::num::NonZeroU32;
use uv_k5_rust_firmware::st7565_mmio_transport::{
    GPIOB_ST7565_A0_MASK, RegisterAddress, RegisterIo, SPI_CR_MSR_SSN_MASK, SPI_FIFOST_TFF_MASK,
    SPI_IF_TX_BUSY_MASK, draw_line_with_io,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Read(RegisterAddress, u32),
    Write(RegisterAddress, u32),
}

struct FakeRegisterIo {
    registers: [u32; 5],
    fifo_status: VecDeque<u32>,
    interrupt_flag: VecDeque<u32>,
    events: Vec<Event>,
}

impl FakeRegisterIo {
    fn new(control: u32, gpio_b_data: u32) -> Self {
        Self {
            registers: [control, 0, 0, 0, gpio_b_data],
            fifo_status: VecDeque::new(),
            interrupt_flag: VecDeque::new(),
            events: Vec::new(),
        }
    }

    fn index(register: RegisterAddress) -> usize {
        match register {
            RegisterAddress::SpiControl => 0,
            RegisterAddress::SpiWriteData => 1,
            RegisterAddress::SpiInterruptFlag => 2,
            RegisterAddress::SpiFifoStatus => 3,
            RegisterAddress::GpioBData => 4,
        }
    }
}

impl RegisterIo for FakeRegisterIo {
    fn read(&mut self, register: RegisterAddress) -> u32 {
        let value = match register {
            RegisterAddress::SpiFifoStatus => self.fifo_status.pop_front().unwrap_or(0),
            RegisterAddress::SpiInterruptFlag => self.interrupt_flag.pop_front().unwrap_or(0),
            _ => self.registers[Self::index(register)],
        };
        self.events.push(Event::Read(register, value));
        value
    }

    fn write(&mut self, register: RegisterAddress, value: u32) {
        self.events.push(Event::Write(register, value));
        self.registers[Self::index(register)] = value;
    }
}

fn poll_limit() -> NonZeroU32 {
    NonZeroU32::new(1).unwrap()
}

#[test]
fn address_map_and_status_masks_match_the_pinned_registers() {
    assert_eq!(
        [
            RegisterAddress::SpiControl.address(),
            RegisterAddress::SpiWriteData.address(),
            RegisterAddress::SpiInterruptFlag.address(),
            RegisterAddress::SpiFifoStatus.address(),
            RegisterAddress::GpioBData.address(),
        ],
        [
            0x400b_8000,
            0x400b_8004,
            0x400b_8014,
            0x400b_8018,
            0x4006_0800,
        ]
    );
    assert_eq!(SPI_CR_MSR_SSN_MASK, 0x0000_1000);
    assert_eq!(GPIOB_ST7565_A0_MASK, 0x0000_0200);
    assert_eq!(SPI_FIFOST_TFF_MASK, 0x0000_0010);
    assert_eq!(SPI_IF_TX_BUSY_MASK, 0x0000_0020);
}

#[test]
fn composed_line_transfer_uses_pinned_registers_and_restores_mode() {
    let mut io = FakeRegisterIo::new(0xa5a5_ffff, 0x1357_9bdf);

    assert_eq!(
        draw_line_with_io(&mut io, 7, 127, &[0xab], poll_limit()),
        Ok(())
    );
    assert_eq!(
        io.events,
        [
            Event::Read(RegisterAddress::SpiControl, 0xa5a5_ffff),
            Event::Write(RegisterAddress::SpiControl, 0xa5a5_efff),
            Event::Read(RegisterAddress::GpioBData, 0x1357_9bdf),
            Event::Write(RegisterAddress::GpioBData, 0x1357_99df),
            Event::Read(RegisterAddress::SpiFifoStatus, 0),
            Event::Write(RegisterAddress::SpiWriteData, 0xb7),
            Event::Read(RegisterAddress::SpiFifoStatus, 0),
            Event::Write(RegisterAddress::SpiWriteData, 0x18),
            Event::Read(RegisterAddress::SpiFifoStatus, 0),
            Event::Write(RegisterAddress::SpiWriteData, 0x03),
            Event::Read(RegisterAddress::SpiInterruptFlag, 0),
            Event::Read(RegisterAddress::GpioBData, 0x1357_99df),
            Event::Write(RegisterAddress::GpioBData, 0x1357_9bdf),
            Event::Read(RegisterAddress::SpiFifoStatus, 0),
            Event::Write(RegisterAddress::SpiWriteData, 0xab),
            Event::Read(RegisterAddress::SpiInterruptFlag, 0),
            Event::Read(RegisterAddress::SpiControl, 0xa5a5_efff),
            Event::Write(RegisterAddress::SpiControl, 0xa5a5_ffff),
        ]
    );
    assert_eq!(io.registers[0] & !0x1000, 0xa5a5_efff & !0x1000);
    assert_eq!(io.registers[4] & !0x200, 0x1357_99df & !0x200);
}

#[test]
fn fifo_timeout_restores_mode_without_writing_after_timeout() {
    let mut io = FakeRegisterIo::new(0xa5a5_ffff, 0x1357_9bdf);
    io.fifo_status = [0, 0x10].into();

    assert_eq!(
        draw_line_with_io(&mut io, 0, 0, &[1], poll_limit()),
        Err(uv_k5_rust_firmware::st7565_mmio_transport::LineError::FifoTimeout)
    );
    assert_eq!(
        io.events,
        [
            Event::Read(RegisterAddress::SpiControl, 0xa5a5_ffff),
            Event::Write(RegisterAddress::SpiControl, 0xa5a5_efff),
            Event::Read(RegisterAddress::GpioBData, 0x1357_9bdf),
            Event::Write(RegisterAddress::GpioBData, 0x1357_99df),
            Event::Read(RegisterAddress::SpiFifoStatus, 0),
            Event::Write(RegisterAddress::SpiWriteData, 0xb0),
            Event::Read(RegisterAddress::SpiFifoStatus, 0x10),
            Event::Read(RegisterAddress::SpiControl, 0xa5a5_efff),
            Event::Write(RegisterAddress::SpiControl, 0xa5a5_ffff),
        ]
    );
}
