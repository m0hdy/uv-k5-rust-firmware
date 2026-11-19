#[path = "../src/portcon_input_plan.rs"]
mod portcon_input_plan;

use portcon_input_plan::{
    Features, PortconInputPlan, PortconInputSnapshot, PortconWrite, Register, WriteKind,
};

fn clear(register: Register, mask: u32) -> (Register, WriteKind) {
    (register, WriteKind::Clear { mask })
}

fn set(register: Register, bits: u32) -> (Register, WriteKind) {
    (register, WriteKind::Set { bits })
}

fn expected_shape(enable_swd: bool) -> [(Register, WriteKind); 20] {
    [
        set(Register::AInputEnable, 0x0000_0178),
        clear(Register::AInputEnable, 0x0000_3c00),
        set(Register::APullUp, 0x0000_0078),
        clear(Register::APullUp, 0x0000_3c00),
        clear(Register::APullDown, 0x0000_3c78),
        set(Register::AOpenDrain, 0x0000_0078),
        clear(Register::AOpenDrain, 0x0000_3c00),
        set(Register::BInputEnable, 0x0000_4000),
        clear(
            Register::BInputEnable,
            if enable_swd { 0x0000_87c0 } else { 0x0000_8fc0 },
        ),
        clear(Register::BPullUp, 0x0000_ca40),
        clear(Register::BPullDown, 0x0000_ca40),
        clear(Register::BOpenDrain, 0x0000_8a40),
        set(Register::BOpenDrain, 0x0000_4000),
        set(Register::CInputEnable, 0x0000_0020),
        clear(Register::CInputEnable, 0x0000_001f),
        set(Register::CPullUp, 0x0000_0020),
        clear(Register::CPullUp, 0x0000_001f),
        clear(Register::CPullDown, 0x0000_003f),
        clear(Register::COpenDrain, 0x0000_001f),
        set(Register::COpenDrain, 0x0000_0020),
    ]
}

fn assert_trace(snapshot: PortconInputSnapshot, enable_swd: bool, expected_values: [u32; 20]) {
    let writes: Vec<PortconWrite> =
        PortconInputPlan::from_snapshot(snapshot, Features { enable_swd })
            .writes()
            .collect();
    let shape = expected_shape(enable_swd);

    assert_eq!(writes.len(), shape.len());
    for (index, ((register, kind), value)) in shape.into_iter().zip(expected_values).enumerate() {
        assert_eq!(
            writes[index],
            PortconWrite {
                register,
                kind,
                value,
            },
            "write {index}"
        );
    }
}

const VARIED_SNAPSHOT: PortconInputSnapshot = PortconInputSnapshot {
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
};

#[test]
fn varied_snapshot_has_literal_ordered_trace_for_both_swd_macro_states() {
    assert_trace(
        VARIED_SNAPSHOT,
        false,
        [
            0xa5a5_5b7a,
            0xa5a5_437a,
            0x1357_9bff,
            0x1357_83ff,
            0xdead_8287,
            0x89ab_cdff,
            0x89ab_c1ff,
            0xfedc_fa98,
            0xfedc_7018,
            0x0123_0527,
            0x7654_3010,
            0x89ab_45af,
            0x89ab_45af,
            0x55aa_55aa,
            0x55aa_55a0,
            0x1234_5678,
            0x1234_5660,
            0xa5a5_a580,
            0xf0f0_f0e0,
            0xf0f0_f0e0,
        ],
    );
    assert_trace(
        VARIED_SNAPSHOT,
        true,
        [
            0xa5a5_5b7a,
            0xa5a5_437a,
            0x1357_9bff,
            0x1357_83ff,
            0xdead_8287,
            0x89ab_cdff,
            0x89ab_c1ff,
            0xfedc_fa98,
            0xfedc_7818,
            0x0123_0527,
            0x7654_3010,
            0x89ab_45af,
            0x89ab_45af,
            0x55aa_55aa,
            0x55aa_55a0,
            0x1234_5678,
            0x1234_5660,
            0xa5a5_a580,
            0xf0f0_f0e0,
            0xf0f0_f0e0,
        ],
    );
}

#[test]
fn zero_snapshot_has_literal_ordered_trace_for_both_swd_macro_states() {
    let snapshot = PortconInputSnapshot {
        porta_ie: 0,
        porta_pu: 0,
        porta_pd: 0,
        porta_od: 0,
        portb_ie: 0,
        portb_pu: 0,
        portb_pd: 0,
        portb_od: 0,
        portc_ie: 0,
        portc_pu: 0,
        portc_pd: 0,
        portc_od: 0,
    };
    let expected = [
        0x0000_0178,
        0x0000_0178,
        0x0000_0078,
        0x0000_0078,
        0x0000_0000,
        0x0000_0078,
        0x0000_0078,
        0x0000_4000,
        0x0000_4000,
        0x0000_0000,
        0x0000_0000,
        0x0000_0000,
        0x0000_4000,
        0x0000_0020,
        0x0000_0020,
        0x0000_0020,
        0x0000_0020,
        0x0000_0000,
        0x0000_0000,
        0x0000_0020,
    ];

    assert_trace(snapshot, false, expected);
    assert_trace(snapshot, true, expected);
}

#[test]
fn all_one_snapshot_has_literal_ordered_trace_for_both_swd_macro_states() {
    let snapshot = PortconInputSnapshot {
        porta_ie: u32::MAX,
        porta_pu: u32::MAX,
        porta_pd: u32::MAX,
        porta_od: u32::MAX,
        portb_ie: u32::MAX,
        portb_pu: u32::MAX,
        portb_pd: u32::MAX,
        portb_od: u32::MAX,
        portc_ie: u32::MAX,
        portc_pu: u32::MAX,
        portc_pd: u32::MAX,
        portc_od: u32::MAX,
    };

    assert_trace(
        snapshot,
        false,
        [
            0xffff_ffff,
            0xffff_c3ff,
            0xffff_ffff,
            0xffff_c3ff,
            0xffff_c387,
            0xffff_ffff,
            0xffff_c3ff,
            0xffff_ffff,
            0xffff_703f,
            0xffff_35bf,
            0xffff_35bf,
            0xffff_75bf,
            0xffff_75bf,
            0xffff_ffff,
            0xffff_ffe0,
            0xffff_ffff,
            0xffff_ffe0,
            0xffff_ffc0,
            0xffff_ffe0,
            0xffff_ffe0,
        ],
    );
    assert_trace(
        snapshot,
        true,
        [
            0xffff_ffff,
            0xffff_c3ff,
            0xffff_ffff,
            0xffff_c3ff,
            0xffff_c387,
            0xffff_ffff,
            0xffff_c3ff,
            0xffff_ffff,
            0xffff_783f,
            0xffff_35bf,
            0xffff_35bf,
            0xffff_75bf,
            0xffff_75bf,
            0xffff_ffff,
            0xffff_ffe0,
            0xffff_ffff,
            0xffff_ffe0,
            0xffff_ffc0,
            0xffff_ffe0,
            0xffff_ffe0,
        ],
    );
}
