# Offline UART1 initialization trace

[`src/uart_init_plan.rs`](../src/uart_init_plan.rs) independently models the
first seven UART1 register writes in `UART_Init()` at
[`driver/uart.c` lines 26–47](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/uart.c#L26-L47)
from `egzumer/uv-k5-firmware-custom` commit
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/commit/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
The RCHF sign and delta field widths, shifts, and masks follow the matching
[`syscon.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/syscon.h#L320-L333);
UART control and FIFO encodings follow
[`uart.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/uart.h#L53-L83).

The sign is bit 31 and the 20-bit RCHF delta occupies bits 11–30. A set sign
adds the extracted delta to 48,000,000; a clear sign subtracts it. The model
uses `u32` wrapping arithmetic to preserve the source's unsigned fixed-width
operations and divides by 39,053 with integer truncation.

| Order | Register | Operation | Value |
| --- | --- | --- | --- |
| 1 | UART1 CTRL | Clear UARTEN, preserve other snapshot bits | Modified snapshot |
| 2 | UART1 BAUD | Replace | Corrected frequency / 39,053 |
| 3 | UART1 CTRL | Replace | RXEN \| TXEN \| RXDMAEN = `0x0000000e` |
| 4 | UART1 RXTO | Replace | `4` |
| 5 | UART1 FC | Replace | `0` |
| 6 | UART1 FIFO | Replace | 8-byte receive level and RX/TX clear = `0x000000c7` |
| 7 | UART1 IE | Replace | `0` |

This is an allocation-free offline transformation of caller-supplied 32-bit
snapshots. It has no public-library wiring, performs no MMIO, and does not
model DMA setup, later UART enablement, serial protocol/access, firmware
programming, sends, startup changes, or device testing. The snapshots are
inputs for reproducible software tests, not captured hardware values.

Focused validation from the repository root:

```sh
cargo test --locked --test uart_init_plan
cargo clippy --locked --test uart_init_plan -- -D warnings
rustfmt --check --edition 2024 src/uart_init_plan.rs tests/uart_init_plan.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/uart_init_plan.rs -o /tmp/uart_init_plan.rlib
```
