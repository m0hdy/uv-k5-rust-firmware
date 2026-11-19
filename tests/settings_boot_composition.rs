#[path = "../src/settings_boot_composition.rs"]
mod settings_boot_composition;

use settings_boot_composition::CoreSettings;
use settings_boot_composition::{
    BacklightMinStatus, Command, DisplayBootDefaults, Features, ReadBackend, ReadPlan,
    SettingsBoot, compose, plan_read,
};

struct Fixtures {
    plans: [ReadPlan; 2],
    responses: [[u8; 8]; 2],
    reads: usize,
}

impl Fixtures {
    fn new(responses: [[u8; 8]; 2]) -> Self {
        Self {
            plans: [plan_read(0, 0); 2],
            responses,
            reads: 0,
        }
    }
}

impl ReadBackend for Fixtures {
    fn read(&mut self, plan: ReadPlan) -> [u8; 8] {
        let index = self.reads;
        self.plans[index] = plan;
        self.reads += 1;
        self.responses[index]
    }
}

const ALL_FEATURES: Features = Features {
    noaa_enabled: true,
    vox_enabled: true,
    blmin_tmp_off_enabled: true,
};

const CORE_DATA: [u8; 8] = [73, 8, 10, 1, 1, 1, 9, 4];
const DISPLAY_DATA: [u8; 8] = [0x24, 3, 1, 4, 2, 5, 1, 1];

const EXPECTED_CORE: CoreSettings = CoreSettings {
    call_channel: 73,
    squelch_level: 8,
    tx_timeout_timer: 10,
    noaa_auto_scan: Some(true),
    key_lock: true,
    vox_switch: Some(true),
    vox_level: Some(9),
    mic_sensitivity: 4,
};

const EXPECTED_DISPLAY: DisplayBootDefaults = DisplayBootDefaults {
    backlight_min: 2,
    backlight_max: 4,
    backlight_min_status: Some(BacklightMinStatus::On),
    channel_display_mode: 3,
    cross_band_rx_tx: 1,
    battery_save: 4,
    dual_watch: 2,
    backlight_time: 5,
    tail_tone_elimination: true,
    vfo_open: true,
};

#[test]
fn reads_two_ordered_blocks_and_routes_each_response_to_its_decoder() {
    let mut fixtures = Fixtures::new([CORE_DATA, DISPLAY_DATA]);

    assert_eq!(
        compose(&mut fixtures, ALL_FEATURES),
        SettingsBoot {
            core: EXPECTED_CORE,
            display: EXPECTED_DISPLAY,
        }
    );
    assert_eq!(fixtures.reads, 2);
    assert_eq!(fixtures.plans, [plan_read(0x0e70, 8), plan_read(0x0e78, 8)]);
    assert_eq!(
        fixtures.plans[0].commands(),
        &[
            Command::Start,
            Command::Write(0xa0),
            Command::Write(0x0e),
            Command::Write(0x70),
            Command::Start,
            Command::Write(0xa1),
            Command::ReadBuffer(8),
            Command::Stop,
        ]
    );
    assert_eq!(
        fixtures.plans[1].commands(),
        &[
            Command::Start,
            Command::Write(0xa0),
            Command::Write(0x0e),
            Command::Write(0x78),
            Command::Start,
            Command::Write(0xa1),
            Command::ReadBuffer(8),
            Command::Stop,
        ]
    );
}

#[test]
fn feature_presence_controls_only_the_corresponding_assignments() {
    for noaa_enabled in [false, true] {
        for vox_enabled in [false, true] {
            for blmin_tmp_off_enabled in [false, true] {
                let mut fixtures = Fixtures::new([CORE_DATA, DISPLAY_DATA]);
                let result = compose(
                    &mut fixtures,
                    Features {
                        noaa_enabled,
                        vox_enabled,
                        blmin_tmp_off_enabled,
                    },
                );

                assert_eq!(result.core.noaa_auto_scan, noaa_enabled.then_some(true));
                assert_eq!(result.core.vox_switch, vox_enabled.then_some(true));
                assert_eq!(result.core.vox_level, vox_enabled.then_some(9));
                assert_eq!(
                    result.display.backlight_min_status,
                    blmin_tmp_off_enabled.then_some(BacklightMinStatus::On)
                );
                assert_eq!(fixtures.reads, 2);
            }
        }
    }
}

#[test]
fn erased_bytes_are_decoded_data_not_a_read_error() {
    let mut fixtures = Fixtures::new([[0xff; 8], [0xff; 8]]);

    assert_eq!(
        compose(&mut fixtures, ALL_FEATURES),
        SettingsBoot {
            core: CoreSettings {
                call_channel: 0,
                squelch_level: 1,
                tx_timeout_timer: 1,
                noaa_auto_scan: Some(false),
                key_lock: false,
                vox_switch: Some(false),
                vox_level: Some(1),
                mic_sensitivity: 4,
            },
            display: DisplayBootDefaults {
                backlight_min: 0,
                backlight_max: 10,
                backlight_min_status: Some(BacklightMinStatus::On),
                channel_display_mode: 0,
                cross_band_rx_tx: 0,
                battery_save: 4,
                dual_watch: 1,
                backlight_time: 3,
                tail_tone_elimination: false,
                vfo_open: true,
            },
        }
    );
    assert_eq!(fixtures.reads, 2);
}
