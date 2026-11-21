#![no_std]

pub use uv_k5_rust_firmware::{
    bk1080_receive_adapter, eeprom_i2c_bridge, eeprom_read_transport, i2c_bitbang_flow,
    settings_boot_composition, settings_extended_boot,
};

#[path = "../../src/settings_channel_acquisition.rs"]
#[allow(unused_attributes)]
pub mod settings_channel_acquisition;

use core::convert::Infallible;
use eeprom_i2c_bridge::I2cBitbangTransport;
use i2c_bitbang_flow::target_gpio::{Delay1us, GpioI2cIo};
use settings_boot_composition::Features;
use settings_channel_acquisition::{SettingsChannelAcquisition, SettingsChannelAcquisitionError};

/// # Safety
///
/// The caller must satisfy the GPIO, pin ownership, register serialization,
/// and delay requirements of `GpioI2cIo::new_target`.
pub unsafe fn acquire_target<D: Delay1us>(
    delay: D,
    features: Features,
) -> Result<SettingsChannelAcquisition, SettingsChannelAcquisitionError<Infallible>> {
    let mut io = unsafe { GpioI2cIo::new_target(delay) };
    let mut transport = I2cBitbangTransport::new(&mut io);
    settings_channel_acquisition::acquire_with_transport(&mut transport, features)
}
