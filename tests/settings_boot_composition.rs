use uv_k5_rust_firmware::settings_boot_composition::CoreSettings;
use uv_k5_rust_firmware::settings_boot_composition::{
    Acknowledgement, BacklightMinStatus, Command, DisplayBootDefaults, Features, I2cTransport,
    ReadBackend, ReadError, ReadPlan, SettingsBoot, SettingsBootBlock, SettingsBootReadError,
    compose, compose_with_transport, plan_read,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BusEvent {
    Start,
    Write(u8),
    RepeatedStart,
    Read(usize),
    Stop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransportError {
    Transfer(usize),
    Cleanup,
}

struct StartupBus {
    events: Vec<BusEvent>,
    responses: [[u8; 8]; 2],
    reads: usize,
    fail_at: Option<usize>,
    transfer_failed: bool,
    fail_cleanup: bool,
}

impl StartupBus {
    fn new(responses: [[u8; 8]; 2]) -> Self {
        Self {
            events: Vec::new(),
            responses,
            reads: 0,
            fail_at: None,
            transfer_failed: false,
            fail_cleanup: false,
        }
    }

    fn record(&mut self, event: BusEvent) -> Result<(), TransportError> {
        let index = self.events.len();
        self.events.push(event);
        if self.fail_at == Some(index) {
            self.transfer_failed = true;
            Err(TransportError::Transfer(index))
        } else if event == BusEvent::Stop && self.transfer_failed && self.fail_cleanup {
            Err(TransportError::Cleanup)
        } else {
            Ok(())
        }
    }
}

impl I2cTransport for StartupBus {
    type Error = TransportError;

    fn start(&mut self) -> Result<(), Self::Error> {
        self.record(BusEvent::Start)
    }

    fn repeated_start(&mut self) -> Result<(), Self::Error> {
        self.record(BusEvent::RepeatedStart)
    }

    fn write_byte(&mut self, byte: u8) -> Result<Acknowledgement, Self::Error> {
        self.record(BusEvent::Write(byte))?;
        Ok(Acknowledgement::Ack)
    }

    fn read_buffer(&mut self, output: &mut [u8]) -> Result<(), Self::Error> {
        let response = self.responses[self.reads];
        self.reads += 1;
        self.record(BusEvent::Read(output.len()))?;
        output.copy_from_slice(&response);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        self.record(BusEvent::Stop)
    }
}

const STARTUP_BUS_TRACE: [BusEvent; 16] = [
    BusEvent::Start,
    BusEvent::Write(0xa0),
    BusEvent::Write(0x0e),
    BusEvent::Write(0x70),
    BusEvent::RepeatedStart,
    BusEvent::Write(0xa1),
    BusEvent::Read(8),
    BusEvent::Stop,
    BusEvent::Start,
    BusEvent::Write(0xa0),
    BusEvent::Write(0x0e),
    BusEvent::Write(0x78),
    BusEvent::RepeatedStart,
    BusEvent::Write(0xa1),
    BusEvent::Read(8),
    BusEvent::Stop,
];

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
fn fallible_entry_executes_both_literal_traces_and_decodes_distinct_responses() {
    let mut bus = StartupBus::new([CORE_DATA, DISPLAY_DATA]);

    assert_eq!(
        compose_with_transport(&mut bus, ALL_FEATURES),
        Ok(SettingsBoot {
            core: EXPECTED_CORE,
            display: EXPECTED_DISPLAY,
        })
    );
    assert_eq!(bus.events, STARTUP_BUS_TRACE);
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
fn fallible_entry_preserves_feature_presence_gates() {
    for noaa_enabled in [false, true] {
        for vox_enabled in [false, true] {
            for blmin_tmp_off_enabled in [false, true] {
                let mut bus = StartupBus::new([CORE_DATA, DISPLAY_DATA]);
                let result = compose_with_transport(
                    &mut bus,
                    Features {
                        noaa_enabled,
                        vox_enabled,
                        blmin_tmp_off_enabled,
                    },
                )
                .unwrap();

                assert_eq!(result.core.noaa_auto_scan, noaa_enabled.then_some(true));
                assert_eq!(result.core.vox_switch, vox_enabled.then_some(true));
                assert_eq!(result.core.vox_level, vox_enabled.then_some(9));
                assert_eq!(
                    result.display.backlight_min_status,
                    blmin_tmp_off_enabled.then_some(BacklightMinStatus::On)
                );
                assert_eq!(bus.events, STARTUP_BUS_TRACE);
            }
        }
    }
}

#[test]
fn erased_bytes_are_decoded_data_not_a_read_error() {
    let erased = [0xff; 8];
    let defaults = SettingsBoot {
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
    };

    for (responses, expected) in [
        ([erased, erased], defaults),
        (
            [erased, DISPLAY_DATA],
            SettingsBoot {
                core: defaults.core,
                display: EXPECTED_DISPLAY,
            },
        ),
        (
            [CORE_DATA, erased],
            SettingsBoot {
                core: EXPECTED_CORE,
                display: defaults.display,
            },
        ),
    ] {
        let mut fixtures = Fixtures::new(responses);
        assert_eq!(
            compose(&mut fixtures, ALL_FEATURES),
            expected,
            "{responses:?}"
        );
        assert_eq!(fixtures.reads, 2);
    }
}

#[test]
fn fallible_entry_decodes_erased_bytes_in_either_block_as_data() {
    let erased = [0xff; 8];
    let erased_defaults = {
        let mut fixtures = Fixtures::new([erased, erased]);
        compose(&mut fixtures, ALL_FEATURES)
    };

    for (responses, expected) in [
        (
            [erased, DISPLAY_DATA],
            SettingsBoot {
                core: erased_defaults.core,
                display: EXPECTED_DISPLAY,
            },
        ),
        (
            [CORE_DATA, erased],
            SettingsBoot {
                core: EXPECTED_CORE,
                display: erased_defaults.display,
            },
        ),
    ] {
        let mut bus = StartupBus::new(responses);
        assert_eq!(compose_with_transport(&mut bus, ALL_FEATURES), Ok(expected));
        assert_eq!(bus.events, STARTUP_BUS_TRACE);
    }
}

#[test]
fn first_block_failure_preserves_transport_error_and_stops_before_second_read() {
    let mut bus = StartupBus::new([CORE_DATA, DISPLAY_DATA]);
    bus.fail_at = Some(2);
    bus.fail_cleanup = true;

    assert_eq!(
        compose_with_transport(&mut bus, ALL_FEATURES),
        Err(SettingsBootReadError {
            block: SettingsBootBlock::Core,
            error: ReadError::Transport(TransportError::Transfer(2)),
        })
    );
    assert_eq!(
        bus.events,
        [
            BusEvent::Start,
            BusEvent::Write(0xa0),
            BusEvent::Write(0x0e),
            BusEvent::Stop,
        ]
    );
}

#[test]
fn second_block_failure_preserves_transport_error_and_cleanup_failure() {
    let mut bus = StartupBus::new([CORE_DATA, DISPLAY_DATA]);
    bus.fail_at = Some(11);
    bus.fail_cleanup = true;

    assert_eq!(
        compose_with_transport(&mut bus, ALL_FEATURES),
        Err(SettingsBootReadError {
            block: SettingsBootBlock::Display,
            error: ReadError::Transport(TransportError::Transfer(11)),
        })
    );
    assert_eq!(
        bus.events,
        [
            BusEvent::Start,
            BusEvent::Write(0xa0),
            BusEvent::Write(0x0e),
            BusEvent::Write(0x70),
            BusEvent::RepeatedStart,
            BusEvent::Write(0xa1),
            BusEvent::Read(8),
            BusEvent::Stop,
            BusEvent::Start,
            BusEvent::Write(0xa0),
            BusEvent::Write(0x0e),
            BusEvent::Write(0x78),
            BusEvent::Stop,
        ]
    );
}
