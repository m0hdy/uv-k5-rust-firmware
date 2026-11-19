#[path = "../src/bk1080_tuning.rs"]
mod bk1080_tuning;

use bk1080_tuning::{
    BandLimits, Command, FrequencyDeviation, Register, TunePlan, band_limits, frequency_deviation,
    mute_command,
};

#[test]
fn band_limits_match_literal_endpoints_and_modulo_selection() {
    assert_eq!(
        band_limits(0),
        BandLimits {
            low: 875,
            high: 1080
        }
    );
    assert_eq!(
        band_limits(1),
        BandLimits {
            low: 760,
            high: 1080
        }
    );
    assert_eq!(
        band_limits(2),
        BandLimits {
            low: 760,
            high: 900
        }
    );
    assert_eq!(
        band_limits(3),
        BandLimits {
            low: 640,
            high: 760
        }
    );
    assert_eq!(
        band_limits(4),
        BandLimits {
            low: 875,
            high: 1080
        }
    );
    assert_eq!(
        band_limits(u8::MAX),
        BandLimits {
            low: 640,
            high: 760
        }
    );
}

#[test]
fn tune_plan_masks_band_bits_and_preserves_every_other_configuration_bit() {
    let plan = TunePlan::new(900, 2, 0xA55A);

    assert_eq!(
        plan.commands().collect::<Vec<_>>(),
        vec![
            Command::WriteRegister {
                register: Register::SystemConfiguration2,
                value: 0xA59A,
            },
            Command::WriteRegister {
                register: Register::Channel,
                value: 140,
            },
            Command::WaitMs(10),
            Command::WriteRegister {
                register: Register::Channel,
                value: 0x808C,
            },
        ]
    );
}

#[test]
fn mute_plan_uses_the_reference_power_configuration_values() {
    assert_eq!(
        mute_command(true),
        Command::WriteRegister {
            register: Register::PowerConfiguration,
            value: 0x4201,
        }
    );
    assert_eq!(
        mute_command(false),
        Command::WriteRegister {
            register: Register::PowerConfiguration,
            value: 0x0201,
        }
    );
}

#[test]
fn tuning_preserves_u16_underflow_and_does_not_reject_out_of_band_values() {
    let below_lower = TunePlan::new(639, 3, 0);
    assert_eq!(
        below_lower.commands().collect::<Vec<_>>(),
        vec![
            Command::WriteRegister {
                register: Register::SystemConfiguration2,
                value: 0x00C0,
            },
            Command::WriteRegister {
                register: Register::Channel,
                value: 0xFFFF,
            },
            Command::WaitMs(10),
            Command::WriteRegister {
                register: Register::Channel,
                value: 0xFFFF,
            },
        ]
    );

    let above_upper = TunePlan::new(761, 3, 0);
    assert_eq!(
        above_upper.commands().nth(1),
        Some(Command::WriteRegister {
            register: Register::Channel,
            value: 121,
        })
    );
}

#[test]
fn deviation_extracts_the_high_twelve_bits_and_keeps_the_base_frequency() {
    assert_eq!(
        frequency_deviation(995, 0xABCD),
        FrequencyDeviation {
            base_frequency: 995,
            deviation: 0x0ABC,
        }
    );
    assert_eq!(
        frequency_deviation(875, 0x000F),
        FrequencyDeviation {
            base_frequency: 875,
            deviation: 0,
        }
    );
}
