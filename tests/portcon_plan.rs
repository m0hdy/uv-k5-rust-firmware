#[path = "../src/portcon_plan.rs"]
mod portcon_plan;

use portcon_plan::{Features, PortconPlan, PortconSnapshot, PortconWrite, Register, WriteKind};

fn clear(register: Register, mask: u32, value: u32) -> PortconWrite {
    PortconWrite {
        register,
        kind: WriteKind::Clear { mask },
        value,
    }
}

fn set(register: Register, bits: u32, value: u32) -> PortconWrite {
    PortconWrite {
        register,
        kind: WriteKind::Set { bits },
        value,
    }
}

const VARIED_SNAPSHOT: PortconSnapshot = PortconSnapshot {
    porta_sel0: 0xa5a5_5a5a,
    porta_sel1: 0x1357_9bdf,
    portb_sel0: 0xdead_beef,
    portb_sel1: 0x89ab_cdef,
    portc_sel0: 0xfedc_ba98,
};

#[test]
fn trace_preserves_every_clear_and_set_operation_in_source_order_without_swd() {
    assert_eq!(
        PortconPlan::from_snapshot(VARIED_SNAPSHOT, Features { enable_swd: false })
            .writes()
            .collect::<Vec<_>>(),
        vec![
            clear(Register::PortASel0, 0x0fff_f000, 0xa000_0a5a),
            set(Register::PortASel0, 0x1000_0000, 0xb000_0a5a),
            clear(Register::PortASel1, 0x00ff_ff00, 0x1300_00df),
            set(Register::PortASel1, 0x0500_0051, 0x1700_00df),
            clear(Register::PortBSel0, 0xf000_0000, 0x0ead_beef),
            set(Register::PortBSel0, 0x1000_0000, 0x1ead_beef),
            clear(Register::PortBSel1, 0xff00_f0f0, 0x00ab_0d0f),
            set(Register::PortBSel1, 0x0000_0101, 0x00ab_0d0f),
            clear(Register::PortCSel0, 0x00ff_ffff, 0xfe00_0000),
        ]
    );
}

#[test]
fn all_one_snapshot_preserves_fields_not_cleared_by_the_source_operations() {
    let writes: Vec<_> = PortconPlan::from_snapshot(
        PortconSnapshot {
            porta_sel0: u32::MAX,
            porta_sel1: u32::MAX,
            portb_sel0: u32::MAX,
            portb_sel1: u32::MAX,
            portc_sel0: u32::MAX,
        },
        Features { enable_swd: false },
    )
    .writes()
    .collect();

    assert_eq!(
        writes,
        vec![
            clear(Register::PortASel0, 0x0fff_f000, 0xf000_0fff),
            set(Register::PortASel0, 0x1000_0000, 0xf000_0fff),
            clear(Register::PortASel1, 0x00ff_ff00, 0xff00_00ff),
            set(Register::PortASel1, 0x0500_0051, 0xff00_00ff),
            clear(Register::PortBSel0, 0xf000_0000, 0x0fff_ffff),
            set(Register::PortBSel0, 0x1000_0000, 0x1fff_ffff),
            clear(Register::PortBSel1, 0xff00_f0f0, 0x00ff_0f0f),
            set(Register::PortBSel1, 0x0000_0101, 0x00ff_0f0f),
            clear(Register::PortCSel0, 0x00ff_ffff, 0xff00_0000),
        ]
    );
}

#[test]
fn swd_variant_sets_b11_and_b14_after_the_same_b1_clear() {
    let writes: Vec<_> = PortconPlan::from_snapshot(VARIED_SNAPSHOT, Features { enable_swd: true })
        .writes()
        .collect();

    assert_eq!(
        writes[6],
        clear(Register::PortBSel1, 0xff00_f0f0, 0x00ab_0d0f)
    );
    assert_eq!(
        writes[7],
        set(Register::PortBSel1, 0x0100_1101, 0x01ab_1d0f)
    );
    assert_eq!(writes.len(), 9);
}

#[test]
fn zero_snapshot_keeps_zero_bit_sets_and_unchanged_clear_operations() {
    let writes: Vec<_> = PortconPlan::from_snapshot(
        PortconSnapshot {
            porta_sel0: 0,
            porta_sel1: 0,
            portb_sel0: 0,
            portb_sel1: 0,
            portc_sel0: 0,
        },
        Features { enable_swd: false },
    )
    .writes()
    .collect();

    assert_eq!(
        writes,
        vec![
            clear(Register::PortASel0, 0x0fff_f000, 0),
            set(Register::PortASel0, 0x1000_0000, 0x1000_0000),
            clear(Register::PortASel1, 0x00ff_ff00, 0),
            set(Register::PortASel1, 0x0500_0051, 0x0500_0051),
            clear(Register::PortBSel0, 0xf000_0000, 0),
            set(Register::PortBSel0, 0x1000_0000, 0x1000_0000),
            clear(Register::PortBSel1, 0xff00_f0f0, 0),
            set(Register::PortBSel1, 0x0000_0101, 0x0000_0101),
            clear(Register::PortCSel0, 0x00ff_ffff, 0),
        ]
    );
}
