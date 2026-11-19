#![cfg_attr(not(test), no_std)]

//! Adapter from EEPROM reads to the offline bit-banged I2C pin flow.

use core::convert::Infallible;

use crate::i2c_bitbang_flow;
use crate::settings_boot_composition;

pub use crate::i2c_bitbang_flow::{Direction, I2cIo, Level};
pub use crate::settings_boot_composition::{
    Acknowledgement, Features, I2cTransport, ReadError, SettingsBoot, SettingsBootBlock,
    SettingsBootReadError,
};

pub struct I2cBitbangTransport<'a, I: I2cIo> {
    io: &'a mut I,
}

impl<'a, I: I2cIo> I2cBitbangTransport<'a, I> {
    pub fn new(io: &'a mut I) -> Self {
        Self { io }
    }
}

impl<I: I2cIo> I2cTransport for I2cBitbangTransport<'_, I> {
    type Error = Infallible;

    fn start(&mut self) -> Result<(), Self::Error> {
        i2c_bitbang_flow::start(self.io);
        Ok(())
    }

    fn repeated_start(&mut self) -> Result<(), Self::Error> {
        i2c_bitbang_flow::start(self.io);
        Ok(())
    }

    fn write_byte(&mut self, byte: u8) -> Result<Acknowledgement, Self::Error> {
        Ok(match i2c_bitbang_flow::write_byte(self.io, byte) {
            i2c_bitbang_flow::Acknowledgement::Ack => Acknowledgement::Ack,
            i2c_bitbang_flow::Acknowledgement::Nack => Acknowledgement::Nack,
        })
    }

    fn read_buffer(&mut self, output: &mut [u8]) -> Result<(), Self::Error> {
        i2c_bitbang_flow::read_buffer(self.io, output);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        i2c_bitbang_flow::stop(self.io);
        Ok(())
    }
}

pub fn read_settings_boot<I: I2cIo>(
    io: &mut I,
    features: Features,
) -> Result<SettingsBoot, SettingsBootReadError<Infallible>> {
    settings_boot_composition::compose_with_transport(&mut I2cBitbangTransport::new(io), features)
}
