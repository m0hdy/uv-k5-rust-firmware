#[allow(unused_attributes)]
#[path = "../src/spi0_plan.rs"]
mod spi0_plan;

use spi0_plan::{LogicalWrite, Register, Spi0InitPlan, Spi0Snapshot, toggle_master_mode};

#[test]
fn spi0_init_has_the_six_literal_writes_in_source_order() {
    let plan = Spi0InitPlan::from_snapshot(Spi0Snapshot {
        control: 0xa5a5_ffff,
        interrupt_enable: 0xffff_ffff,
        device_clock_gate: 0xa5a5_51aa,
    });

    assert_eq!(
        plan.writes().collect::<Vec<_>>(),
        vec![
            LogicalWrite {
                register: Register::Control,
                value: 0xa5a5_fff7,
            },
            LogicalWrite {
                register: Register::DeviceClockGate,
                value: 0xa5a5_55aa,
            },
            LogicalWrite {
                register: Register::Control,
                value: 0xa5a5_fff7,
            },
            LogicalWrite {
                register: Register::Control,
                value: 0xa5a5_7f72,
            },
            LogicalWrite {
                register: Register::InterruptEnable,
                value: 0,
            },
            LogicalWrite {
                register: Register::Control,
                value: 0xa5a5_7f7a,
            },
        ]
    );
}

#[test]
fn configuration_preserves_unaffected_bits_including_tf_clr() {
    let snapshot = Spi0Snapshot {
        control: 0x1357_9bff,
        interrupt_enable: 0x1234_5678,
        device_clock_gate: 0x2468_ace1,
    };
    let writes: Vec<_> = Spi0InitPlan::from_snapshot(snapshot).writes().collect();
    let control = writes[3].value;
    let configuration_mask = 0x0000_00f7 | (1 << 15);

    assert_eq!(
        control & !configuration_mask,
        0x1357_9bf7 & !configuration_mask
    );
    assert_eq!(control & 0x7, 2);
    assert_eq!(control & (1 << 4), 1 << 4);
    assert_eq!(control & (1 << 5), 1 << 5);
    assert_eq!(control & (1 << 6), 1 << 6);
    assert_eq!(control & (1 << 7), 0);
    assert_eq!(control & (1 << 15), 0);
    assert_eq!(control & (1 << 16), 1 << 16);
}

#[test]
fn clock_gate_sets_only_the_spi0_enable_bit() {
    for device_clock_gate in [0, 0xffff_ffff, 0xa5a5_51aa] {
        let writes: Vec<_> = Spi0InitPlan::from_snapshot(Spi0Snapshot {
            control: 0,
            interrupt_enable: 0,
            device_clock_gate,
        })
        .writes()
        .collect();
        assert_eq!(writes[1].register, Register::DeviceClockGate);
        assert_eq!(writes[1].value & (1 << 10), 1 << 10);
        assert_eq!(writes[1].value & !(1 << 10), device_clock_gate & !(1 << 10));
    }
}

#[test]
fn initialization_clears_interrupt_enables_without_an_nvic_write() {
    let writes: Vec<_> = Spi0InitPlan::from_snapshot(Spi0Snapshot {
        control: 0,
        interrupt_enable: 0xffff_ffff,
        device_clock_gate: 0,
    })
    .writes()
    .collect();

    assert_eq!(writes.len(), 6);
    assert_eq!(writes[4].register, Register::InterruptEnable);
    assert_eq!(writes[4].value, 0);
    assert!(
        !writes
            .iter()
            .any(|write| write.register == Register::InterruptEnable && write.value != 0)
    );
}

#[test]
fn master_mode_toggle_sets_or_clears_only_msr_ssn() {
    let control = 0xa5a5_ffff;

    assert_eq!(
        toggle_master_mode(control & !(1 << 12), true),
        LogicalWrite {
            register: Register::Control,
            value: control | (1 << 12),
        }
    );
    assert_eq!(
        toggle_master_mode(control, false),
        LogicalWrite {
            register: Register::Control,
            value: control & !(1 << 12),
        }
    );
}
