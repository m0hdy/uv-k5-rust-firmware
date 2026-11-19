#[path = "../src/board_startup_composition.rs"]
mod board_startup_composition;

use board_startup_composition::{
    Call, Defines, Event, Features, GpioFeatures, GpioPlan, GpioPort, GpioRegister, GpioSnapshot,
    InputRegister, InputWriteKind, PortconInputFeatures, PortconInputPlan, PortconInputSnapshot,
    PortconPlan, PortconSelFeatures, PortconSnapshot, SelRegister, SelWrite, SelWriteKind,
    Snapshots, compose,
};

const VARIED: Snapshots = Snapshots {
    portcon_sel: PortconSnapshot {
        porta_sel0: 0xa5a5_5a5a,
        porta_sel1: 0x1357_9bdf,
        portb_sel0: 0xdead_beef,
        portb_sel1: 0x89ab_cdef,
        portc_sel0: 0xfedc_ba98,
    },
    portcon_input: PortconInputSnapshot {
        porta_ie: 0xa5a5_5a5a,
        porta_pu: 0x1357_9bdf,
        porta_pd: 0xdead_beef,
        porta_od: 0x89ab_cdef,
        portb_ie: 0xfedc_ba98,
        portb_pu: 0x0123_4567,
        portb_pd: 0x7654_3210,
        portb_od: 0x89ab_cdef,
        portc_ie: 0x55aa_55aa,
        portc_pu: 0x1234_5678,
        portc_pd: 0xa5a5_a5a5,
        portc_od: 0xf0f0_f0f0,
    },
    gpio: GpioSnapshot {
        direction: [0xa5a5_5a5a, 0x1357_9bdf, 0xdead_beef],
        data: [0x0123_4567, 0x89ab_4def, 0xfedc_ba98],
    },
};

const SECOND: Snapshots = Snapshots {
    portcon_sel: PortconSnapshot {
        porta_sel0: 0,
        porta_sel1: u32::MAX,
        portb_sel0: 0x55aa_55aa,
        portb_sel1: 0,
        portc_sel0: u32::MAX,
    },
    portcon_input: PortconInputSnapshot {
        porta_ie: 0,
        porta_pu: u32::MAX,
        porta_pd: 0x55aa_55aa,
        porta_od: u32::MAX,
        portb_ie: 0,
        portb_pu: u32::MAX,
        portb_pd: 0x55aa_55aa,
        portb_od: u32::MAX,
        portc_ie: 0,
        portc_pu: u32::MAX,
        portc_pd: 0x55aa_55aa,
        portc_od: u32::MAX,
    },
    gpio: GpioSnapshot {
        direction: [u32::MAX, 0, 0x55aa_55aa],
        data: [u32::MAX, 0, 0x55aa_55aa],
    },
};

fn defines(enable_fmradio: bool, enable_uart: bool, enabled_aircopy: bool) -> Defines {
    Defines {
        enable_fmradio,
        enable_uart,
        enabled_aircopy,
    }
}

fn selected_writes(trace: &[Event]) -> Vec<board_startup_composition::SelWrite> {
    trace
        .iter()
        .filter_map(|event| match event {
            Event::PortconSel(write) => Some(*write),
            _ => None,
        })
        .collect()
}

fn input_writes(trace: &[Event]) -> Vec<board_startup_composition::InputWrite> {
    trace
        .iter()
        .filter_map(|event| match event {
            Event::PortconInput(write) => Some(*write),
            _ => None,
        })
        .collect()
}

fn gpio_updates(trace: &[Event]) -> Vec<board_startup_composition::GpioUpdate> {
    trace
        .iter()
        .filter_map(|event| match event {
            Event::Gpio(update) => Some(*update),
            _ => None,
        })
        .collect()
}

fn assert_sel_writes_preserve_unrelated_bits(snapshot: PortconSnapshot, writes: &[SelWrite]) {
    let mut values = [
        snapshot.porta_sel0,
        snapshot.porta_sel1,
        snapshot.portb_sel0,
        snapshot.portb_sel1,
        snapshot.portc_sel0,
    ];
    for write in writes {
        let index = match write.register {
            SelRegister::PortASel0 => 0,
            SelRegister::PortASel1 => 1,
            SelRegister::PortBSel0 => 2,
            SelRegister::PortBSel1 => 3,
            SelRegister::PortCSel0 => 4,
        };
        let touched = match write.kind {
            SelWriteKind::Clear { mask } => mask,
            SelWriteKind::Set { bits } => bits,
        };
        assert_eq!((values[index] ^ write.value) & !touched, 0);
        values[index] = write.value;
    }
}

fn assert_input_writes_preserve_unrelated_bits(
    snapshot: PortconInputSnapshot,
    writes: &[board_startup_composition::InputWrite],
) {
    let mut values = [
        snapshot.porta_ie,
        snapshot.portb_ie,
        snapshot.portc_ie,
        snapshot.porta_pu,
        snapshot.portb_pu,
        snapshot.portc_pu,
        snapshot.porta_pd,
        snapshot.portb_pd,
        snapshot.portc_pd,
        snapshot.porta_od,
        snapshot.portb_od,
        snapshot.portc_od,
    ];
    for write in writes {
        let index = match write.register {
            InputRegister::AInputEnable => 0,
            InputRegister::BInputEnable => 1,
            InputRegister::CInputEnable => 2,
            InputRegister::APullUp => 3,
            InputRegister::BPullUp => 4,
            InputRegister::CPullUp => 5,
            InputRegister::APullDown => 6,
            InputRegister::BPullDown => 7,
            InputRegister::CPullDown => 8,
            InputRegister::AOpenDrain => 9,
            InputRegister::BOpenDrain => 10,
            InputRegister::COpenDrain => 11,
        };
        let touched = match write.kind {
            InputWriteKind::Clear { mask } => mask,
            InputWriteKind::Set { bits } => bits,
        };
        assert_eq!((values[index] ^ write.value) & !touched, 0);
        values[index] = write.value;
    }
}

#[test]
fn plan_writes_and_mock_calls_share_the_pinned_startup_order() {
    for snapshots in [VARIED, SECOND] {
        let flags = Features {
            defines: defines(true, true, false),
            enable_swd: true,
        };
        let trace = compose(snapshots, flags);
        let events = trace.events();

        assert_eq!(
            &events[..2],
            [
                Event::Call(Call::DeviceClockGates),
                Event::Call(Call::SystickInit),
            ]
        );
        let sel = selected_writes(events);
        assert_eq!(
            sel,
            PortconPlan::from_snapshot(
                snapshots.portcon_sel,
                PortconSelFeatures { enable_swd: true },
            )
            .writes()
            .collect::<Vec<_>>()
        );
        assert_sel_writes_preserve_unrelated_bits(snapshots.portcon_sel, &sel);
        let input = input_writes(events);
        assert_eq!(
            input,
            PortconInputPlan::from_snapshot(
                snapshots.portcon_input,
                PortconInputFeatures { enable_swd: true },
            )
            .writes()
            .collect::<Vec<_>>()
        );
        assert_input_writes_preserve_unrelated_bits(snapshots.portcon_input, &input);
        assert_eq!(
            gpio_updates(events),
            GpioPlan::for_features(GpioFeatures {
                enable_fmradio: true,
            })
            .updates()
            .collect::<Vec<_>>()
        );

        let first_input = events
            .iter()
            .position(|event| matches!(event, Event::PortconInput(_)))
            .unwrap();
        let first_gpio = events
            .iter()
            .position(|event| matches!(event, Event::Gpio(_)))
            .unwrap();
        let backlight = events
            .iter()
            .position(|event| *event == Event::Call(Call::BacklightInitHardware))
            .unwrap();
        assert!(
            events[2..first_input]
                .iter()
                .all(|event| matches!(event, Event::PortconSel(_)))
        );
        assert!(
            events[first_input..first_gpio]
                .iter()
                .all(|event| matches!(event, Event::PortconInput(_)))
        );
        assert!(
            events[first_gpio..backlight]
                .iter()
                .all(|event| matches!(event, Event::Gpio(_)))
        );
        assert_eq!(
            &events[backlight..backlight + 3],
            [
                Event::Call(Call::BacklightInitHardware),
                Event::Call(Call::AdcInit),
                Event::Call(Call::St7565Init),
            ]
        );
        assert_eq!(
            &events[backlight + 3..],
            [Event::Call(Call::Bk1080Init0), Event::Call(Call::CrcInit)]
        );
    }
}

#[test]
fn swd_and_fm_features_reach_only_their_respective_plan_phases() {
    for enable_swd in [false, true] {
        for enable_fmradio in [false, true] {
            for (enable_uart, enabled_aircopy) in
                [(false, false), (true, false), (false, true), (true, true)]
            {
                let trace = compose(
                    VARIED,
                    Features {
                        defines: defines(enable_fmradio, enable_uart, enabled_aircopy),
                        enable_swd,
                    },
                );
                let sel = selected_writes(trace.events());
                let input = input_writes(trace.events());
                let gpio = gpio_updates(trace.events());

                assert_eq!(
                    sel[7].kind,
                    SelWriteKind::Set {
                        bits: if enable_swd { 0x0100_1101 } else { 0x0000_0101 },
                    }
                );
                assert_eq!(
                    input[8].kind,
                    board_startup_composition::InputWriteKind::Clear {
                        mask: if enable_swd { 0x0000_87c0 } else { 0x0000_8fc0 },
                    }
                );
                assert_eq!(gpio.len(), if enable_fmradio { 6 } else { 5 });
                assert_eq!(
                    gpio.last().map(|update| update.register),
                    if enable_fmradio {
                        Some(GpioRegister::Data)
                    } else {
                        Some(GpioRegister::Direction)
                    }
                );
                assert_eq!(
                    trace
                        .events()
                        .iter()
                        .filter(|event| **event == Event::Call(Call::Bk1080Init0))
                        .count(),
                    usize::from(enable_fmradio)
                );
                assert_eq!(trace.gpio_after().data[1] & 0x8000 != 0, enable_fmradio);
                assert_eq!(
                    trace
                        .events()
                        .iter()
                        .filter(|event| **event == Event::Call(Call::CrcInit))
                        .count(),
                    usize::from(enable_uart || enabled_aircopy)
                );
            }
        }
    }
}

#[test]
fn composed_gpio_application_preserves_bits_untouched_by_the_gpio_plan() {
    for snapshots in [VARIED, SECOND] {
        for enable_fmradio in [false, true] {
            let trace = compose(
                snapshots,
                Features {
                    defines: defines(enable_fmradio, false, false),
                    enable_swd: false,
                },
            );
            let mut expected = snapshots.gpio;
            let plan = GpioPlan::for_features(GpioFeatures { enable_fmradio });
            plan.apply(&mut expected);
            assert_eq!(trace.gpio_after(), expected);

            let mut touched_direction = [0; 3];
            let mut touched_data = [0; 3];
            for update in plan.updates() {
                let touched = match update.register {
                    GpioRegister::Direction => &mut touched_direction,
                    GpioRegister::Data => &mut touched_data,
                };
                let port = match update.port {
                    GpioPort::A => 0,
                    GpioPort::B => 1,
                    GpioPort::C => 2,
                };
                touched[port] |= update.mask;
            }

            for port in 0..3 {
                assert_eq!(
                    (snapshots.gpio.direction[port] ^ expected.direction[port])
                        & !touched_direction[port],
                    0
                );
                assert_eq!(
                    (snapshots.gpio.data[port] ^ expected.data[port]) & !touched_data[port],
                    0
                );
            }
        }
    }
}
