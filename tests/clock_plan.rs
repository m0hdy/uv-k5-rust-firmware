#[path = "../src/clock_plan.rs"]
mod clock_plan;

use clock_plan::{ClockPlan, ClockSnapshot, ClockWrite, Register, WriteKind};

#[test]
fn clock_plan_matches_the_three_literal_writes_in_order() {
    let plan = ClockPlan::from_snapshot(ClockSnapshot {
        pmu_src_cfg: 0xa5a5_5aff,
        syscon_clk_sel: 0xffff_ffff,
        syscon_div_clk_gate: 0xdead_beef,
    });

    assert_eq!(
        plan.writes().collect::<Vec<_>>(),
        vec![
            ClockWrite {
                register: Register::PmuSrcCfg,
                kind: WriteKind::Modify {
                    clear_mask: 0x0000_0003,
                    set_bits: 0x0000_0001,
                },
                value: 0xa5a5_5afd,
            },
            ClockWrite {
                register: Register::SysconClkSel,
                kind: WriteKind::Replace,
                value: 0x0000_0002,
            },
            ClockWrite {
                register: Register::SysconDivClkGate,
                kind: WriteKind::Modify {
                    clear_mask: 0x0000_0001,
                    set_bits: 0,
                },
                value: 0xdead_beee,
            },
        ]
    );
}

#[test]
fn read_modify_write_values_preserve_bits_outside_the_fields() {
    let snapshot = ClockSnapshot {
        pmu_src_cfg: 0x1357_9bde,
        syscon_clk_sel: 0x8765_4321,
        syscon_div_clk_gate: 0x2468_ace1,
    };
    let writes: Vec<_> = ClockPlan::from_snapshot(snapshot).writes().collect();

    assert_eq!(writes[0].value & !0x3, snapshot.pmu_src_cfg & !0x3);
    assert_eq!(writes[0].value & 0x3, 0x1);
    assert_eq!(writes[2].value & !0x1, snapshot.syscon_div_clk_gate & !0x1);
    assert_eq!(writes[2].value & 0x1, 0);
}

#[test]
fn clock_selection_is_replaced_without_inheriting_snapshot_bits() {
    for syscon_clk_sel in [0, 0xffff_ffff, 0xa5a5_5a5a] {
        let plan = ClockPlan::from_snapshot(ClockSnapshot {
            pmu_src_cfg: 0,
            syscon_clk_sel,
            syscon_div_clk_gate: 0,
        });
        let writes: Vec<_> = plan.writes().collect();
        assert_eq!(writes[1].register, Register::SysconClkSel);
        assert_eq!(writes[1].kind, WriteKind::Replace);
        assert_eq!(writes[1].value, 0x2);
    }
}
