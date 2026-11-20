#![cfg_attr(not(test), no_std)]

use crate::startup_version_display::StartupDisplayMode;
use crate::{display_boot_defaults, settings_boot_defaults};

use crate::eeprom_read_transport::execute_read;
pub use crate::eeprom_read_transport::{
    Acknowledgement, Command, I2cTransport, ReadError, ReadPlan, plan_read,
};
pub use display_boot_defaults::{BacklightMinStatus, DisplayBootDefaults};
pub use settings_boot_defaults::CoreSettings;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Features {
    pub noaa_enabled: bool,
    pub vox_enabled: bool,
    pub blmin_tmp_off_enabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettingsBoot {
    pub core: CoreSettings,
    pub display: DisplayBootDefaults,
    pub welcome_mode: StartupDisplayMode,
}

pub trait ReadBackend {
    fn read(&mut self, plan: ReadPlan) -> [u8; 8];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsBootBlock {
    Core,
    Display,
    Welcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettingsBootReadError<E> {
    pub block: SettingsBootBlock,
    pub error: ReadError<E>,
}

pub fn compose<B: ReadBackend>(backend: &mut B, features: Features) -> SettingsBoot {
    let core_data = backend.read(plan_read(
        settings_boot_defaults::EEPROM_OFFSET,
        settings_boot_defaults::EEPROM_LEN as u8,
    ));
    let core = decode_core(core_data, features);

    let display_data = backend.read(plan_read(
        display_boot_defaults::EEPROM_OFFSET,
        display_boot_defaults::EEPROM_LEN as u8,
    ));
    let display = decode_display(display_data, features);
    let welcome_data = backend.read(plan_read(0x0e90, 8));
    let welcome_mode = decode_welcome_mode(welcome_data);

    SettingsBoot {
        core,
        display,
        welcome_mode,
    }
}

pub fn compose_with_transport<T: I2cTransport>(
    transport: &mut T,
    features: Features,
) -> Result<SettingsBoot, SettingsBootReadError<T::Error>> {
    let mut core_data = [0; 8];
    execute_read(
        transport,
        plan_read(
            settings_boot_defaults::EEPROM_OFFSET,
            settings_boot_defaults::EEPROM_LEN as u8,
        ),
        &mut core_data,
    )
    .map_err(|error| SettingsBootReadError {
        block: SettingsBootBlock::Core,
        error,
    })?;
    let core = decode_core(core_data, features);

    let mut display_data = [0; 8];
    execute_read(
        transport,
        plan_read(
            display_boot_defaults::EEPROM_OFFSET,
            display_boot_defaults::EEPROM_LEN as u8,
        ),
        &mut display_data,
    )
    .map_err(|error| SettingsBootReadError {
        block: SettingsBootBlock::Display,
        error,
    })?;
    let display = decode_display(display_data, features);

    let mut welcome_data = [0; 8];
    execute_read(transport, plan_read(0x0e90, 8), &mut welcome_data).map_err(|error| {
        SettingsBootReadError {
            block: SettingsBootBlock::Welcome,
            error,
        }
    })?;
    let welcome_mode = decode_welcome_mode(welcome_data);

    Ok(SettingsBoot {
        core,
        display,
        welcome_mode,
    })
}

const fn decode_welcome_mode(data: [u8; 8]) -> StartupDisplayMode {
    match data[7] {
        0 => StartupDisplayMode::FullScreen,
        1 => StartupDisplayMode::Custom,
        2 => StartupDisplayMode::Voltage,
        3 => StartupDisplayMode::None,
        _ => StartupDisplayMode::Voltage,
    }
}

fn decode_core(data: [u8; 8], features: Features) -> CoreSettings {
    settings_boot_defaults::decode_core_settings(
        data,
        settings_boot_defaults::FeaturePresence {
            noaa_enabled: features.noaa_enabled,
            vox_enabled: features.vox_enabled,
        },
    )
}

fn decode_display(data: [u8; 8], features: Features) -> DisplayBootDefaults {
    display_boot_defaults::decode_display_boot_defaults(
        data,
        display_boot_defaults::FeaturePresence {
            blmin_tmp_off_enabled: features.blmin_tmp_off_enabled,
        },
    )
}
