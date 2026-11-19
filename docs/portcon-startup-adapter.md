# PortCon startup register adapter

`src/portcon_startup_adapter.rs` applies the ordered operations from
[`portcon_plan.rs`](../src/portcon_plan.rs) and
[`portcon_input_plan.rs`](../src/portcon_input_plan.rs) through a mockable
register-I/O boundary. It is not exported from `src/lib.rs` and has no boot
call site.

## Pinned source and address map

The source behavior is pinned to
[`egzumer/uv-k5-firmware-custom@7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
`BOARD_PORTCON_Init()` performs the SEL operations in
[`board.c:116–225`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/board.c#L116-L225),
followed by input-enable, pull-up, pull-down, and open-drain operations in
[`board.c:226–455`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/board.c#L226-L455).
The literal PORTCON base and register offsets are declared in the matching
[`bsp/dp32g030/portcon.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h#L25-L28)
and the per-register definitions:

| Register | Address | Pinned definition |
| --- | --- | --- |
| PORTA SEL0 | `0x400b0000` | [`PORTCON_PORTA_SEL0_ADDR`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h#L28) |
| PORTA SEL1 | `0x400b0004` | [`PORTCON_PORTA_SEL1_ADDR`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h#L130) |
| PORTB SEL0 | `0x400b0008` | [`PORTCON_PORTB_SEL0_ADDR`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h#L260) |
| PORTB SEL1 | `0x400b000c` | [`PORTCON_PORTB_SEL1_ADDR`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h#L374) |
| PORTC SEL0 | `0x400b0010` | [`PORTCON_PORTC_SEL0_ADDR`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h#L490) |
| PORTA/B/C IE | `0x400b0100` / `0x400b0104` / `0x400b0108` | [`PORTCON_PORTA_IE_ADDR`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h#L612), [`PORTCON_PORTB_IE_ADDR`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h#L742), [`PORTCON_PORTC_IE_ADDR`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h#L872) |
| PORTA/B/C PU | `0x400b0200` / `0x400b0204` / `0x400b0208` | [`PORTCON_PORTA_PU_ADDR`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h#L1002), [`PORTCON_PORTB_PU_ADDR`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h#L1132), [`PORTCON_PORTC_PU_ADDR`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h#L1262) |
| PORTA/B/C PD | `0x400b0300` / `0x400b0304` / `0x400b0308` | [`PORTCON_PORTA_PD_ADDR`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h#L1392), [`PORTCON_PORTB_PD_ADDR`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h#L1522), [`PORTCON_PORTC_PD_ADDR`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h#L1652) |
| PORTA/B/C OD | `0x400b0400` / `0x400b0404` / `0x400b0408` | [`PORTCON_PORTA_OD_ADDR`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h#L1782), [`PORTCON_PORTB_OD_ADDR`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h#L1912), [`PORTCON_PORTC_OD_ADDR`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h#L2042) |

The address type is closed over these seventeen aligned 32-bit registers; no
caller-supplied address is used. The adapter obtains clear masks and set bits
from the existing typed plans. It does not duplicate their bit tables or
replay their snapshot-derived values: each operation reads the current
register value and applies only that operation's mask or bits.

The SEL plan runs first in its existing nine-operation order. The input plan
then runs in its existing twenty-operation order (A, B, then C; IE, PU, PD,
then OD within each port). Both plans are instantiated with zero snapshots
only to obtain their typed operation sequences. The only `ENABLE_SWD` changes
are those already represented by the plans: PORTB SEL1 set bits and the
PORTB IE clear mask.

All 29 pinned PortCon operations are source compound assignments (`|=` or
`&= ~`), so each one performs a fresh read followed by a write, including
operations whose result is unchanged. There are no replacement writes in
these plans. A later update to the same register reads the value written by
the preceding update rather than reusing an earlier snapshot.

## Target ownership and scope

The ARM bare-metal backend is available only under `target_arch = "arm"` and
`target_os = "none"` and uses volatile 32-bit reads and writes. The unsafe
entry point requires a DP32G030 with the PORTCON mapping above and accessible
registers. It must run with exclusive startup ownership: interrupts, threads,
and other code must not modify the affected PORTCON registers during the
read-modify-write sequence.

This adapter is a standalone software boundary. It is not wired into boot,
does not publish firmware, and makes no hardware-execution or bench claim.
Host tests use a mock register backend and do not access devices.

Focused validation from the repository root:

```sh
cargo test --locked --test portcon_startup_adapter
cargo clippy --locked --test portcon_startup_adapter -- -D warnings
rustfmt --check --edition 2024 \
  src/portcon_startup_adapter.rs tests/portcon_startup_adapter.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings -C opt-level=2 --emit=asm src/portcon_startup_adapter.rs \
  -o /tmp/portcon_startup_adapter.s
```

Inspect the emitted assembly for retained 32-bit `ldr` and `str` accesses.
This target compile and assembly inspection do not execute the adapter or
establish hardware behavior.
