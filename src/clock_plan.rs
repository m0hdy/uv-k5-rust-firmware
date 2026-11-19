//! Offline model of `SYSTEM_ConfigureClocks` at pinned reference commit
//! `7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`
//! ([`system.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/system.c),
//! [PMU fields](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/pmu.h),
//! and [SYSCON fields](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/syscon.h)).
//! This independently authored plan operates only on supplied software
//! snapshots; it performs no MMIO or hardware execution.
//!
//! The C comment says “Set source clock from external crystal,” but the encoded
//! fields select and enable RCHF 48 MHz. This model records those fields only;
//! it makes no claim about oscillator readiness, timing, inherited clock state,
//! or safe board sequencing.

const PMU_SRC_CFG_RCHF_FIELDS_MASK: u32 = 0b11;
const PMU_SRC_CFG_RCHF_SEL_BITS_48MHZ: u32 = 0;
const PMU_SRC_CFG_RCHF_EN_BITS_ENABLE: u32 = 1;
const SYSCON_CLK_SEL_DIV_BITS_2: u32 = 1 << 1;
const SYSCON_DIV_CLK_GATE_DIV_CLK_GATE_MASK: u32 = 1;
const SYSCON_DIV_CLK_GATE_DIV_CLK_GATE_BITS_DISABLE: u32 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Register {
    PmuSrcCfg,
    SysconClkSel,
    SysconDivClkGate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteKind {
    Modify { clear_mask: u32, set_bits: u32 },
    Replace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClockWrite {
    pub register: Register,
    pub kind: WriteKind,
    pub value: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClockSnapshot {
    pub pmu_src_cfg: u32,
    pub syscon_clk_sel: u32,
    pub syscon_div_clk_gate: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClockPlan {
    writes: [ClockWrite; 3],
}

impl ClockPlan {
    pub const fn from_snapshot(snapshot: ClockSnapshot) -> Self {
        Self {
            writes: [
                ClockWrite {
                    register: Register::PmuSrcCfg,
                    kind: WriteKind::Modify {
                        clear_mask: PMU_SRC_CFG_RCHF_FIELDS_MASK,
                        set_bits: PMU_SRC_CFG_RCHF_SEL_BITS_48MHZ | PMU_SRC_CFG_RCHF_EN_BITS_ENABLE,
                    },
                    value: (snapshot.pmu_src_cfg & !PMU_SRC_CFG_RCHF_FIELDS_MASK)
                        | PMU_SRC_CFG_RCHF_SEL_BITS_48MHZ
                        | PMU_SRC_CFG_RCHF_EN_BITS_ENABLE,
                },
                ClockWrite {
                    register: Register::SysconClkSel,
                    kind: WriteKind::Replace,
                    value: SYSCON_CLK_SEL_DIV_BITS_2,
                },
                ClockWrite {
                    register: Register::SysconDivClkGate,
                    kind: WriteKind::Modify {
                        clear_mask: SYSCON_DIV_CLK_GATE_DIV_CLK_GATE_MASK,
                        set_bits: SYSCON_DIV_CLK_GATE_DIV_CLK_GATE_BITS_DISABLE,
                    },
                    value: (snapshot.syscon_div_clk_gate & !SYSCON_DIV_CLK_GATE_DIV_CLK_GATE_MASK)
                        | SYSCON_DIV_CLK_GATE_DIV_CLK_GATE_BITS_DISABLE,
                },
            ],
        }
    }

    pub fn writes(&self) -> impl Iterator<Item = ClockWrite> + '_ {
        self.writes.iter().copied()
    }
}
