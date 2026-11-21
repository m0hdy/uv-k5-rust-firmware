#![cfg_attr(not(test), no_std)]

#[path = "channel_selection_boot_defaults.rs"]
#[allow(unused_attributes)]
mod channel_selection_boot_defaults;

use crate::eeprom_read_transport::{I2cTransport, ReadError, execute_read, plan_read};
use crate::settings_boot_composition::{
    Features, SettingsBoot, SettingsBootReadError, compose_with_transport,
};
use crate::settings_extended_boot::{
    CHANNEL_ATTRIBUTES_LEN, CHANNEL_ATTRIBUTES_OFFSET, CUSTOM_AES_KEY_LEN, CUSTOM_AES_KEY_OFFSET,
    CustomAesKey, DTMF_ANI_OFFSET, DTMF_DOWN_OFFSET, DTMF_KILL_OFFSET, DTMF_REVIVE_OFFSET,
    DTMF_UP_OFFSET, DtmfCodeSettings, EEPROM_OFFSETS, ExtendedBootFeatures, ExtendedBootSettings,
    FM_CHANNEL_COUNT, FM_CHANNELS_OFFSET, FM_CONFIG_OFFSET, FmBootSettings,
    PersistedChannelAttributes, decode_channel_attributes, decode_custom_aes_key,
    decode_dtmf_codes, decode_extended_boot, decode_fm_boot,
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
    pub fm: Option<FmBootSettings>,
    pub extended: ExtendedBootSettings,
    pub dtmf_codes: DtmfCodeSettings,
    pub channel_attributes: [PersistedChannelAttributes; CHANNEL_ATTRIBUTES_LEN],
    pub custom_aes_key: CustomAesKey,
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
/// the standard extended blocks and optional FM state, and decodes them as one
/// snapshot. Calibration remains separate because it depends on device state
/// and includes hardware setup.
pub fn acquire_full_portable_with_transport<T: I2cTransport>(
    transport: &mut T,
    features: Features,
    extended_features: ExtendedBootFeatures,
) -> Result<FullPortableSettingsAcquisition, FullPortableSettingsAcquisitionError<T::Error>> {
    let mut base = acquire_with_transport(transport, features)
        .map_err(FullPortableSettingsAcquisitionError::Base)?;
    let fm = if extended_features.fm_enabled {
        let mut config = [0; 4];
        execute_read(transport, plan_read(FM_CONFIG_OFFSET, 4), &mut config).map_err(|error| {
            FullPortableSettingsAcquisitionError::Extended {
                offset: FM_CONFIG_OFFSET,
                error,
            }
        })?;
        let mut channels = [0; FM_CHANNEL_COUNT * 2];
        execute_read(
            transport,
            plan_read(FM_CHANNELS_OFFSET, (FM_CHANNEL_COUNT * 2) as u8),
            &mut channels,
        )
        .map_err(|error| FullPortableSettingsAcquisitionError::Extended {
            offset: FM_CHANNELS_OFFSET,
            error,
        })?;
        Some(decode_fm_boot(config, channels))
    } else {
        None
    };
    let mut blocks = [[0; 8]; EEPROM_OFFSETS.len()];
    for (index, offset) in EEPROM_OFFSETS.iter().copied().enumerate().take(6) {
        execute_read(transport, plan_read(offset, 8), &mut blocks[index])
            .map_err(|error| FullPortableSettingsAcquisitionError::Extended { offset, error })?;
    }

    let calling_codes = if extended_features.dtmf_calling_enabled {
        let mut codes = [[0; 8]; 3];
        for (index, offset) in [DTMF_ANI_OFFSET, DTMF_KILL_OFFSET, DTMF_REVIVE_OFFSET]
            .iter()
            .copied()
            .enumerate()
        {
            execute_read(transport, plan_read(offset, 8), &mut codes[index]).map_err(|error| {
                FullPortableSettingsAcquisitionError::Extended { offset, error }
            })?;
        }
        Some(codes)
    } else {
        None
    };

    let mut up_code = [0; 16];
    execute_read(transport, plan_read(DTMF_UP_OFFSET, 16), &mut up_code).map_err(|error| {
        FullPortableSettingsAcquisitionError::Extended {
            offset: DTMF_UP_OFFSET,
            error,
        }
    })?;
    let mut down_code = [0; 16];
    execute_read(transport, plan_read(DTMF_DOWN_OFFSET, 16), &mut down_code).map_err(|error| {
        FullPortableSettingsAcquisitionError::Extended {
            offset: DTMF_DOWN_OFFSET,
            error,
        }
    })?;

    for (index, offset) in EEPROM_OFFSETS.iter().copied().enumerate().skip(6) {
        execute_read(transport, plan_read(offset, 8), &mut blocks[index])
            .map_err(|error| FullPortableSettingsAcquisitionError::Extended { offset, error })?;
    }
    let mut attributes_data = [0; CHANNEL_ATTRIBUTES_LEN];
    execute_read(
        transport,
        plan_read(CHANNEL_ATTRIBUTES_OFFSET, CHANNEL_ATTRIBUTES_LEN as u8),
        &mut attributes_data,
    )
    .map_err(|error| FullPortableSettingsAcquisitionError::Extended {
        offset: CHANNEL_ATTRIBUTES_OFFSET,
        error,
    })?;
    let mut aes_key_data = [0; CUSTOM_AES_KEY_LEN];
    execute_read(
        transport,
        plan_read(CUSTOM_AES_KEY_OFFSET, CUSTOM_AES_KEY_LEN as u8),
        &mut aes_key_data,
    )
    .map_err(|error| FullPortableSettingsAcquisitionError::Extended {
        offset: CUSTOM_AES_KEY_OFFSET,
        error,
    })?;
    if !base.settings.display.vfo_open {
        base.channel_selection.screen_channel = base.channel_selection.mr_channel;
    }
    Ok(FullPortableSettingsAcquisition {
        base,
        fm,
        extended: decode_extended_boot(blocks, extended_features),
        dtmf_codes: decode_dtmf_codes(calling_codes, up_code, down_code),
        channel_attributes: decode_channel_attributes(attributes_data),
        custom_aes_key: decode_custom_aes_key(aes_key_data),
    })
}
