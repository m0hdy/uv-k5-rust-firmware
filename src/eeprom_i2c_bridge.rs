#![cfg_attr(not(test), no_std)]

//! Adapter from EEPROM reads to the offline bit-banged I2C pin flow.

use core::convert::Infallible;

#[path = "i2c_bitbang_flow.rs"]
#[allow(dead_code, unused_attributes)]
mod i2c_bitbang_flow;
#[path = "settings_boot_composition.rs"]
#[allow(dead_code, unused_attributes, unused_imports)]
mod settings_boot_composition;

pub use i2c_bitbang_flow::{Direction, I2cIo, Level};
pub use settings_boot_composition::{
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

impl<I: I2cIo> settings_boot_composition::I2cTransport for I2cBitbangTransport<'_, I> {
    type Error = Infallible;

    fn start(&mut self) -> Result<(), Self::Error> {
        i2c_bitbang_flow::start(self.io);
        Ok(())
    }

    fn repeated_start(&mut self) -> Result<(), Self::Error> {
        i2c_bitbang_flow::start(self.io);
        Ok(())
    }

    fn write_byte(
        &mut self,
        byte: u8,
    ) -> Result<settings_boot_composition::Acknowledgement, Self::Error> {
        Ok(match i2c_bitbang_flow::write_byte(self.io, byte) {
            i2c_bitbang_flow::Acknowledgement::Ack => {
                settings_boot_composition::Acknowledgement::Ack
            }
            i2c_bitbang_flow::Acknowledgement::Nack => {
                settings_boot_composition::Acknowledgement::Nack
            }
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
