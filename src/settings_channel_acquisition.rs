#![cfg_attr(not(test), no_std)]

#[path = "channel_selection_boot_defaults.rs"]
#[allow(unused_attributes)]
mod channel_selection_boot_defaults;

use crate::eeprom_read_transport::{I2cTransport, ReadError, execute_read, plan_read};
use crate::settings_boot_composition::{
    Features, SettingsBoot, SettingsBootReadError, compose_with_transport,
};
use crate::settings_extended_boot::{
    EEPROM_OFFSETS, ExtendedBootFeatures, ExtendedBootSettings, decode_extended_boot,
};

pub use channel_selection_boot_defaults::ChannelSelectionBootDefaults;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettingsChannelAcquisition {
    pub settings: SettingsBoot,
    pub channel_selection: ChannelSelectionBootDefaults,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsChannelAcquisitionError<E> {
    SettingsBoot(SettingsBootReadError<E>),
    ChannelSelection(ReadError<E>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FullPortableSettingsAcquisition {
    pub base: SettingsChannelAcquisition,
    pub extended: ExtendedBootSettings,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FullPortableSettingsAcquisitionError<E> {
    Base(SettingsChannelAcquisitionError<E>),
    Extended { offset: u16, error: ReadError<E> },
}

pub fn acquire_with_transport<T: I2cTransport>(
    transport: &mut T,
    features: Features,
) -> Result<SettingsChannelAcquisition, SettingsChannelAcquisitionError<T::Error>> {
    let settings = compose_with_transport(transport, features)
        .map_err(SettingsChannelAcquisitionError::SettingsBoot)?;

    let mut channel_data = [0; channel_selection_boot_defaults::EEPROM_LEN];
    execute_read(
        transport,
        plan_read(
            channel_selection_boot_defaults::EEPROM_OFFSET,
            channel_selection_boot_defaults::EEPROM_LEN as u8,
        ),
        &mut channel_data,
    )
    .map_err(SettingsChannelAcquisitionError::ChannelSelection)?;

    let channel_selection = channel_selection_boot_defaults::decode_channel_selection_boot_defaults(
        channel_data,
        features.noaa_enabled,
    );

    Ok(SettingsChannelAcquisition {
        settings,
        channel_selection,
    })
}

/// Acquires the existing core/display/channel-selection sequence followed by
/// each standard extended settings block, and decodes them as one snapshot.
/// Optional FM storage and calibration are separate because their layouts and
/// side effects depend on target features and hardware.
pub fn acquire_full_portable_with_transport<T: I2cTransport>(
    transport: &mut T,
    features: Features,
    extended_features: ExtendedBootFeatures,
) -> Result<FullPortableSettingsAcquisition, FullPortableSettingsAcquisitionError<T::Error>> {
    let base = acquire_with_transport(transport, features)
        .map_err(FullPortableSettingsAcquisitionError::Base)?;
    let mut blocks = [[0; 8]; EEPROM_OFFSETS.len()];
    for (index, offset) in EEPROM_OFFSETS.iter().copied().enumerate() {
        execute_read(transport, plan_read(offset, 8), &mut blocks[index])
            .map_err(|error| FullPortableSettingsAcquisitionError::Extended { offset, error })?;
    }
    Ok(FullPortableSettingsAcquisition {
        base,
        extended: decode_extended_boot(blocks, extended_features),
    })
}
