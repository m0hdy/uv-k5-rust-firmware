#![cfg_attr(not(test), no_std)]

//! Offline model of the UART1 setup in `UART_Init` at pinned reference commit
//! `7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`
//! ([`uart.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/uart.c#L26-L47),
//! [SYSCON fields](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/syscon.h#L320-L333),
//! and [UART fields](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/uart.h#L53-L83)).
//! This records seven ordered writes from supplied snapshots. It performs no
//! MMIO and does not model DMA, later UART enablement, or serial I/O.

const SYSCON_RC_FREQ_DELTA_RCHF_DELTA_SHIFT: u32 = 11;
const SYSCON_RC_FREQ_DELTA_RCHF_DELTA_MASK: u32 = 0x7fff_f800;
const SYSCON_RC_FREQ_DELTA_RCHF_SIG_SHIFT: u32 = 31;
const SYSCON_RC_FREQ_DELTA_RCHF_SIG_MASK: u32 = 0x8000_0000;

const UART_CTRL_UARTEN_MASK: u32 = 1 << 0;
const UART_CTRL_UARTEN_BITS_DISABLE: u32 = 0;
const UART_CTRL_RXEN_BITS_ENABLE: u32 = 1 << 1;
const UART_CTRL_TXEN_BITS_ENABLE: u32 = 1 << 2;
const UART_CTRL_RXDMAEN_BITS_ENABLE: u32 = 1 << 3;
const UART_FIFO_RF_LEVEL_BITS_8_BYTE: u32 = 7;
const UART_FIFO_RF_CLR_BITS_ENABLE: u32 = 1 << 6;
const UART_FIFO_TF_CLR_BITS_ENABLE: u32 = 1 << 7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Register {
    Ctrl,
    Baud,
    RxTimeout,
    FlowControl,
    Fifo,
    InterruptEnable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteKind {
    Modify { clear_mask: u32, set_bits: u32 },
    Replace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UartWrite {
    pub register: Register,
    pub kind: WriteKind,
    pub value: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UartInitSnapshot {
    pub syscon_rc_freq_delta: u32,
    pub uart1_ctrl: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UartInitPlan {
    writes: [UartWrite; 7],
}

impl UartInitPlan {
    pub const fn from_snapshot(snapshot: UartInitSnapshot) -> Self {
        let positive = (snapshot.syscon_rc_freq_delta & SYSCON_RC_FREQ_DELTA_RCHF_SIG_MASK)
            >> SYSCON_RC_FREQ_DELTA_RCHF_SIG_SHIFT
            != 0;
        let delta = (snapshot.syscon_rc_freq_delta & SYSCON_RC_FREQ_DELTA_RCHF_DELTA_MASK)
            >> SYSCON_RC_FREQ_DELTA_RCHF_DELTA_SHIFT;
        let frequency = if positive {
            48_000_000_u32.wrapping_add(delta)
        } else {
            48_000_000_u32.wrapping_sub(delta)
        };
        let baud = frequency / 39_053;
        let initial_ctrl =
            (snapshot.uart1_ctrl & !UART_CTRL_UARTEN_MASK) | UART_CTRL_UARTEN_BITS_DISABLE;

        Self {
            writes: [
                UartWrite {
                    register: Register::Ctrl,
                    kind: WriteKind::Modify {
                        clear_mask: UART_CTRL_UARTEN_MASK,
                        set_bits: UART_CTRL_UARTEN_BITS_DISABLE,
                    },
                    value: initial_ctrl,
                },
                UartWrite {
                    register: Register::Baud,
                    kind: WriteKind::Replace,
                    value: baud,
                },
                UartWrite {
                    register: Register::Ctrl,
                    kind: WriteKind::Replace,
                    value: UART_CTRL_RXEN_BITS_ENABLE
                        | UART_CTRL_TXEN_BITS_ENABLE
                        | UART_CTRL_RXDMAEN_BITS_ENABLE,
                },
                UartWrite {
                    register: Register::RxTimeout,
                    kind: WriteKind::Replace,
                    value: 4,
                },
                UartWrite {
                    register: Register::FlowControl,
                    kind: WriteKind::Replace,
                    value: 0,
                },
                UartWrite {
                    register: Register::Fifo,
                    kind: WriteKind::Replace,
                    value: UART_FIFO_RF_LEVEL_BITS_8_BYTE
                        | UART_FIFO_RF_CLR_BITS_ENABLE
                        | UART_FIFO_TF_CLR_BITS_ENABLE,
                },
                UartWrite {
                    register: Register::InterruptEnable,
                    kind: WriteKind::Replace,
                    value: 0,
                },
            ],
        }
    }

    pub fn writes(&self) -> impl Iterator<Item = UartWrite> + '_ {
        self.writes.iter().copied()
    }
}
