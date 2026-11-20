# Bounded UART reply executor

[`src/uart_reply_executor.rs`](../src/uart_reply_executor.rs) implements the
outbound framing in `SendReply()` and byte-write ordering in `UART_Send()` from
[`app/uart.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/app/uart.c#L171-L199)
and [`driver/uart.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/uart.c#L78-L87)
at commit
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/commit/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
It models replies only; command parsing, receive/DMA setup, entry wiring, and
container encoding are outside this module.

## Wire behavior

The executor rejects payload slices longer than `u16::MAX` before changing the
slice or calling the backend. For accepted slices it optionally XORs every
caller byte in place, including the inner four-byte reply header, using the
16-byte `Obfuscation` sequence from `app/uart.c`. It then invokes three
separate send boundaries in order:

1. `AB CD`, followed by the payload length as a 16-bit little-endian value.
2. The caller's payload slice, including an empty slice.
3. Two padding bytes followed by `DC BA`.

Plaintext padding is `FF FF`. Encrypted padding is
`Obfuscation[size % 16] ^ FF` and `Obfuscation[(size + 1) % 16] ^ FF`.
No reply CRC is generated. In encrypted mode, the caller's payload remains
mutated even if a later FIFO poll times out.

For each byte, the executor writes UART1 TDR first, then reads IF until
`UART_IF_TXFIFO_FULL_MASK` (`0x4000`) is clear or the caller-supplied nonzero
poll limit is exhausted. Each poll is a fresh IF read; unrelated IF bits are
ignored and the IF register is not written or cleared by this transport. This
preserves the pinned `UART_Send()` behavior while bounding its otherwise
unbounded wait. A timeout reports the section and cumulative number of bytes
written to TDR, including the last write before timeout. This count describes
software submissions, not bytes physically delivered. The executor stops at
the first timeout and does not retry.

## Target access

The target entry uses volatile 32-bit accesses at UART1 base `0x4006B800`, TDR
`0x4006B808`, and IF `0x4006B814`. These follow the UART1 base, `UART_Port_t`
field offsets, and TX FIFO full mask in the pinned
[`bsp/dp32g030/uart.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/uart.h#L29-L51)
and its FIFO flag definitions
([lines 269–275](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/uart.h#L269-L275)).

Before calling the unsafe MMIO entry, the caller must enable the UART clock,
initialize/enable UART1 for transmit, configure its TX pin mux, and guarantee
the register mapping. The caller must exclusively own and serialize access to
TDR and IF throughout the call, including against interrupts. The entry does
not perform initialization or arbitration; backend access must not panic.

## Scope and validation

The host backend makes the framing and register polling trace-testable without
hardware. The target fixture is compiled only and never executed. No firmware
entry integration, UART initialization, hardware execution, flashing,
bootloader work, RF/device acceptance, or installable release is included.

From the repository root:

```sh
cargo test --locked --test uart_reply_executor
cargo clippy --locked --test uart_reply_executor -- -D warnings
rustfmt --check --edition 2024 \
  src/uart_reply_executor.rs tests/uart_reply_executor.rs \
  tests/fixtures/uart_reply_executor_target.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings tests/fixtures/uart_reply_executor_target.rs \
  -o /tmp/uart_reply_executor_target.rlib
```
