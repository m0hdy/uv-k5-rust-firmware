#![cfg_attr(not(test), no_std)]

#[path = "display_boot_defaults.rs"]
#[allow(unused_attributes)]
mod display_boot_defaults;
#[path = "eeprom_transaction.rs"]
#[allow(dead_code, unused_attributes)]
mod eeprom_transaction;
#[path = "settings_boot_defaults.rs"]
#[allow(unused_attributes)]
mod settings_boot_defaults;

pub use display_boot_defaults::{BacklightMinStatus, DisplayBootDefaults};
pub use eeprom_transaction::{Command, ReadPlan, plan_read};
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
}

pub trait ReadBackend {
    fn read(&mut self, plan: ReadPlan) -> [u8; 8];
}

pub fn compose<B: ReadBackend>(backend: &mut B, features: Features) -> SettingsBoot {
    let core_data = backend.read(plan_read(
        settings_boot_defaults::EEPROM_OFFSET,
        settings_boot_defaults::EEPROM_LEN as u8,
    ));
    let core = settings_boot_defaults::decode_core_settings(
        core_data,
        settings_boot_defaults::FeaturePresence {
            noaa_enabled: features.noaa_enabled,
            vox_enabled: features.vox_enabled,
        },
    );

    let display_data = backend.read(plan_read(
        display_boot_defaults::EEPROM_OFFSET,
        display_boot_defaults::EEPROM_LEN as u8,
    ));
    let display = display_boot_defaults::decode_display_boot_defaults(
        display_data,
        display_boot_defaults::FeaturePresence {
            blmin_tmp_off_enabled: features.blmin_tmp_off_enabled,
        },
    );

    SettingsBoot { core, display }
}
