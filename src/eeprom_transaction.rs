#![cfg_attr(not(test), no_std)]

//! Pure transaction decisions modeled from the pinned
//! [`EEPROM_ReadBuffer` and `EEPROM_WriteBuffer`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/eeprom.c#L24-L62).
//! This module describes commands only; it performs no bus, delay, or EEPROM
//! access.
//!
//! The read plan always emits the legacy command sequence, including for a
//! zero-byte read or any 16-bit address. The write plan models the legacy
//! null-data and `address >= 0x2000` guards, compares exactly eight supplied
//! current bytes, and waits eight milliseconds after a changed write. It does
//! not add a full-range check: address `0x1fff` remains allowed even though
//! eight bytes extend beyond `0x2000`. Existing bytes are supplied by the
//! caller rather than read from EEPROM here.

pub const EEPROM_WRITE_SIZE: usize = 8;
pub const EEPROM_WRITE_DELAY_MS: u8 = 8;
pub const EEPROM_ADDRESS_LIMIT: u16 = 0x2000;

const EEPROM_WRITE_DEVICE: u8 = 0xa0;
const EEPROM_READ_DEVICE: u8 = 0xa1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    Start,
    Write(u8),
    ReadBuffer(u8),
    Stop,
    DelayMs(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadPlan {
    commands: [Command; 8],
}

impl ReadPlan {
    pub const fn commands(&self) -> &[Command; 8] {
        &self.commands
    }
}

pub const fn plan_read(address: u16, size: u8) -> ReadPlan {
    ReadPlan {
        commands: [
            Command::Start,
            Command::Write(EEPROM_WRITE_DEVICE),
            Command::Write((address >> 8) as u8),
            Command::Write(address as u8),
            Command::Start,
            Command::Write(EEPROM_READ_DEVICE),
            Command::ReadBuffer(size),
            Command::Stop,
        ],
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WritePlan {
    commands: [Command; EEPROM_WRITE_SIZE + 6],
}

impl WritePlan {
    pub const fn commands(&self) -> &[Command; EEPROM_WRITE_SIZE + 6] {
        &self.commands
    }
}

pub fn plan_write(
    address: u16,
    data: Option<&[u8; EEPROM_WRITE_SIZE]>,
    current: &[u8; EEPROM_WRITE_SIZE],
) -> Option<WritePlan> {
    let data = data?;
    if address >= EEPROM_ADDRESS_LIMIT || data == current {
        return None;
    }

    let mut commands = [Command::Stop; EEPROM_WRITE_SIZE + 6];
    commands[0] = Command::Start;
    commands[1] = Command::Write(EEPROM_WRITE_DEVICE);
    commands[2] = Command::Write((address >> 8) as u8);
    commands[3] = Command::Write(address as u8);
    let mut index = 0;
    while index < EEPROM_WRITE_SIZE {
        commands[index + 4] = Command::Write(data[index]);
        index += 1;
    }
    commands[EEPROM_WRITE_SIZE + 4] = Command::Stop;
    commands[EEPROM_WRITE_SIZE + 5] = Command::DelayMs(EEPROM_WRITE_DELAY_MS);

    Some(WritePlan { commands })
}
