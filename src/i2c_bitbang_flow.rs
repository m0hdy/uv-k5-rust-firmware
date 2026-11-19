#![cfg_attr(not(test), no_std)]

//! Offline model of the pinned I2C bit and bus-edge sequence.

pub mod target_gpio;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    Low,
    High,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Input,
    Output,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Acknowledgement {
    Ack,
    Nack,
}

/// Infallible operations supplied by an offline mock, not a hardware driver.
pub trait I2cIo {
    fn scl(&mut self, level: Level);
    fn sda(&mut self, level: Level);
    fn sda_input_enable(&mut self, enabled: bool);
    fn sda_open_drain(&mut self, enabled: bool);
    fn sda_direction(&mut self, direction: Direction);
    fn read_sda(&mut self) -> Level;
    fn delay_1us(&mut self);
}

fn input(io: &mut impl I2cIo) {
    io.sda_input_enable(true);
    io.sda_open_drain(false);
    io.sda_direction(Direction::Input);
}

fn output(io: &mut impl I2cIo) {
    io.sda_input_enable(false);
    io.sda_open_drain(true);
    io.sda_direction(Direction::Output);
}

pub fn start(io: &mut impl I2cIo) {
    io.sda(Level::High);
    io.delay_1us();
    io.scl(Level::High);
    io.delay_1us();
    io.sda(Level::Low);
    io.delay_1us();
    io.scl(Level::Low);
    io.delay_1us();
}

pub fn stop(io: &mut impl I2cIo) {
    io.sda(Level::Low);
    io.delay_1us();
    io.scl(Level::Low);
    io.delay_1us();
    io.scl(Level::High);
    io.delay_1us();
    io.sda(Level::High);
    io.delay_1us();
}

pub fn write_byte(io: &mut impl I2cIo, byte: u8) -> Acknowledgement {
    io.scl(Level::Low);
    io.delay_1us();
    for bit in (0..8).rev() {
        io.sda(if byte & (1 << bit) == 0 {
            Level::Low
        } else {
            Level::High
        });
        io.delay_1us();
        io.scl(Level::High);
        io.delay_1us();
        io.scl(Level::Low);
        io.delay_1us();
    }

    input(io);
    io.sda(Level::High);
    io.delay_1us();
    io.scl(Level::High);
    io.delay_1us();
    let mut result = Acknowledgement::Nack;
    for _ in 0..255 {
        if io.read_sda() == Level::Low {
            result = Acknowledgement::Ack;
            break;
        }
    }
    io.scl(Level::Low);
    io.delay_1us();
    output(io);
    io.sda(Level::High);
    result
}

/// Reads one byte and transmits ACK for a non-final byte or NACK for a final byte.
pub fn read_byte(io: &mut impl I2cIo, final_byte: bool) -> u8 {
    input(io);
    let mut byte = 0u8;
    for _ in 0..8 {
        io.scl(Level::Low);
        io.delay_1us();
        io.scl(Level::High);
        io.delay_1us();
        byte <<= 1;
        io.delay_1us();
        if io.read_sda() == Level::High {
            byte |= 1;
        }
        io.scl(Level::Low);
        io.delay_1us();
    }
    output(io);
    io.scl(Level::Low);
    io.delay_1us();
    io.sda(if final_byte { Level::High } else { Level::Low });
    io.delay_1us();
    io.scl(Level::High);
    io.delay_1us();
    io.scl(Level::Low);
    io.delay_1us();
    byte
}

/// Reads into a slice, ACKing all but the final byte. An empty slice does no I/O.
pub fn read_buffer(io: &mut impl I2cIo, output: &mut [u8]) {
    let len = output.len();
    for (index, byte) in output.iter_mut().enumerate() {
        io.delay_1us();
        *byte = read_byte(io, index == len - 1);
    }
}
