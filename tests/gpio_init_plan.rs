#[path = "../src/gpio_init_plan.rs"]
mod gpio_init_plan;

use gpio_init_plan::{
    Features, GpioPlan, GpioSnapshot, Operation, PINNED_DEFAULT_FEATURES, Port, Register, Update,
};

fn update(port: Port, register: Register, operation: Operation, mask: u32) -> Update {
    Update {
        port,
        register,
        operation,
        mask,
    }
}

#[test]
fn pinned_default_features_produce_the_literal_startup_trace_in_order() {
    assert_eq!(
        PINNED_DEFAULT_FEATURES,
        Features {
            enable_fmradio: true
        }
    );

    let plan = GpioPlan::for_features(PINNED_DEFAULT_FEATURES);
    assert_eq!(
        plan.updates().collect::<Vec<_>>(),
        vec![
            update(Port::A, Register::Direction, Operation::SetBits, 0x3c00),
            update(Port::A, Register::Direction, Operation::ClearBits, 0x0078),
            update(Port::B, Register::Direction, Operation::SetBits, 0x8a00),
            update(Port::C, Register::Direction, Operation::SetBits, 0x001f),
            update(Port::C, Register::Direction, Operation::ClearBits, 0x0020),
            update(Port::B, Register::Data, Operation::SetBits, 0x8000),
        ]
    );
}

#[test]
fn applying_the_default_plan_sets_directions_and_only_the_fm_radio_startup_level() {
    let mut snapshot = GpioSnapshot {
        direction: [0; 3],
        data: [0; 3],
    };

    GpioPlan::for_features(PINNED_DEFAULT_FEATURES).apply(&mut snapshot);

    assert_eq!(snapshot.direction, [0x3c00, 0x8a00, 0x001f]);
    assert_eq!(snapshot.data, [0, 0x8000, 0]);
}

#[test]
fn updates_preserve_unrelated_direction_and_data_bits() {
    let before = GpioSnapshot {
        direction: [0xa5a5_5a5a, 0x1357_9bdf, 0xdead_beef],
        data: [0x0123_4567, 0x89ab_cdef, 0xfedc_ba98],
    };
    let mut after = before;

    GpioPlan::for_features(PINNED_DEFAULT_FEATURES).apply(&mut after);

    assert_eq!(after.direction[0], (before.direction[0] | 0x3c00) & !0x0078);
    assert_eq!(after.direction[1], before.direction[1] | 0x8a00);
    assert_eq!(after.direction[2], (before.direction[2] | 0x001f) & !0x0020);
    assert_eq!(after.data[0], before.data[0]);
    assert_eq!(after.data[1], before.data[1] | 0x8000);
    assert_eq!(after.data[2], before.data[2]);

    for (before, after, touched) in [
        (before.direction[0], after.direction[0], 0x3c78),
        (before.direction[1], after.direction[1], 0x8a00),
        (before.direction[2], after.direction[2], 0x003f),
        (before.data[0], after.data[0], 0),
        (before.data[1], after.data[1], 0x8000),
        (before.data[2], after.data[2], 0),
    ] {
        assert_eq!((before ^ after) & !touched, 0);
    }
}

#[test]
fn disabled_fmradio_omits_the_data_write_and_preserves_every_output_level() {
    let plan = GpioPlan::for_features(Features {
        enable_fmradio: false,
    });
    assert_eq!(
        plan.updates().collect::<Vec<_>>(),
        vec![
            update(Port::A, Register::Direction, Operation::SetBits, 0x3c00),
            update(Port::A, Register::Direction, Operation::ClearBits, 0x0078),
            update(Port::B, Register::Direction, Operation::SetBits, 0x8a00),
            update(Port::C, Register::Direction, Operation::SetBits, 0x001f),
            update(Port::C, Register::Direction, Operation::ClearBits, 0x0020),
        ]
    );

    let mut snapshot = GpioSnapshot {
        direction: [0; 3],
        data: [0x1234, 0x8000, 0x5678],
    };
    let initial_data = snapshot.data;
    plan.apply(&mut snapshot);
    assert_eq!(snapshot.data, initial_data);
}
