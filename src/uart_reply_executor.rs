#![cfg_attr(not(test), no_std)]

//! Bounded execution of the pinned UART `SendReply` framing and FIFO writes.

use core::num::NonZeroU32;

pub const UART1_BASE: usize = 0x4006_b800;
pub const UART1_TDR: usize = UART1_BASE + 0x08;
pub const UART1_IF: usize = UART1_BASE + 0x14;
pub const UART_IF_TXFIFO_FULL_MASK: u32 = 0x4000;

const OBFUSCATION: [u8; 16] = [
    0x16, 0x6c, 0x14, 0xe6, 0x2e, 0x91, 0x0d, 0x40, 0x21, 0x35, 0xd5, 0x40, 0x13, 0x03, 0xe9, 0x80,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendPhase {
    Header,
    Payload,
    Footer,
}

/// Infallible, non-panicking operations used by one UART reply.
///
/// `begin_uart_send` is called once at each C `UART_Send` boundary with the
/// already-transformed reply payload, including the empty payload section. It
/// has no target-side register effect.
pub trait FifoBackend {
    fn begin_uart_send(&mut self, phase: SendPhase, reply_payload: &[u8]);
    fn write_tdr(&mut self, byte: u8);
    fn read_if(&mut self) -> u32;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplyError {
    PayloadTooLarge {
        len: usize,
    },
    FifoTimeout {
        phase: SendPhase,
        bytes_submitted: u32,
    },
}

fn send_section<B: FifoBackend>(
    backend: &mut B,
    phase: SendPhase,
    bytes: &[u8],
    reply_payload: &[u8],
    poll_limit: NonZeroU32,
    bytes_submitted: &mut u32,
) -> Result<(), ReplyError> {
    backend.begin_uart_send(phase, reply_payload);
    for &byte in bytes {
        backend.write_tdr(byte);
        *bytes_submitted += 1;

        let mut fifo_full = true;
        for _ in 0..poll_limit.get() {
            fifo_full = backend.read_if() & UART_IF_TXFIFO_FULL_MASK != 0;
            if !fifo_full {
                break;
            }
        }
        if fifo_full {
            return Err(ReplyError::FifoTimeout {
                phase,
                bytes_submitted: *bytes_submitted,
            });
        }
    }
    Ok(())
}

/// XOR-mutates and sends a caller-owned reply using the pinned framing.
///
/// The payload length is checked before mutation or backend access. When
/// `encrypted` is true, the complete payload (including its inner header) is
/// XORed in place before the first UART section is sent. A timeout does not
/// restore the payload or retry any bytes.
pub fn send_reply<B: FifoBackend>(
    backend: &mut B,
    payload: &mut [u8],
    encrypted: bool,
    poll_limit: NonZeroU32,
) -> Result<(), ReplyError> {
    let size = u16::try_from(payload.len())
        .map_err(|_| ReplyError::PayloadTooLarge { len: payload.len() })?;

    if encrypted {
        for (index, byte) in payload.iter_mut().enumerate() {
            *byte ^= OBFUSCATION[index % OBFUSCATION.len()];
        }
    }

    let header = [0xab, 0xcd, size as u8, (size >> 8) as u8];
    let footer_padding = if encrypted {
        [
            OBFUSCATION[usize::from(size) % OBFUSCATION.len()] ^ 0xff,
            OBFUSCATION[(usize::from(size) + 1) % OBFUSCATION.len()] ^ 0xff,
        ]
    } else {
        [0xff, 0xff]
    };
    let footer = [footer_padding[0], footer_padding[1], 0xdc, 0xba];

    let mut bytes_submitted = 0;
    send_section(
        backend,
        SendPhase::Header,
        &header,
        payload,
        poll_limit,
        &mut bytes_submitted,
    )?;
    send_section(
        backend,
        SendPhase::Payload,
        payload,
        payload,
        poll_limit,
        &mut bytes_submitted,
    )?;
    send_section(
        backend,
        SendPhase::Footer,
        &footer,
        payload,
        poll_limit,
        &mut bytes_submitted,
    )
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct VolatileUart1;

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl FifoBackend for VolatileUart1 {
    fn begin_uart_send(&mut self, _phase: SendPhase, _reply_payload: &[u8]) {}

    fn write_tdr(&mut self, byte: u8) {
        // SAFETY: send_reply_mmio requires the mapped, aligned UART1 registers.
        unsafe { core::ptr::write_volatile(UART1_TDR as *mut u32, u32::from(byte)) }
    }

    fn read_if(&mut self) -> u32 {
        // SAFETY: send_reply_mmio requires the mapped, aligned UART1 registers.
        unsafe { core::ptr::read_volatile(UART1_IF as *const u32) }
    }
}

/// Executes one reply using volatile UART1 MMIO accesses.
///
/// # Safety
///
/// The caller must guarantee that the DP32G030 UART1 mapping matches
/// [`UART1_BASE`], UART1's clock is enabled, UART1 is initialized and enabled
/// for transmit, and its TX pin is muxed to UART1. This function does not
/// configure clocks, muxes, or UART registers. The caller must have exclusive,
/// serialized ownership of UART1 TDR and IF for the entire call, including
/// against interrupts and other writers. Backend accesses must remain
/// non-panicking. The polling limit bounds each post-write FIFO-full check;
/// it does not establish that submitted bytes were physically delivered.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn send_reply_mmio(
    payload: &mut [u8],
    encrypted: bool,
    poll_limit: NonZeroU32,
) -> Result<(), ReplyError> {
    send_reply(&mut VolatileUart1, payload, encrypted, poll_limit)
}
