# Offline PortCon SEL write plan

`src/portcon_plan.rs` independently models only the initial SEL register
operations in
[`BOARD_PORTCON_Init()`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/board.c#L116-L225)
from `egzumer/uv-k5-firmware-custom@7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`.
Field positions, masks, and function encodings follow the matching
[DP32G030 PORTCON definitions](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h):
each SEL field is four bits, A3–A6 GPIO encodings are zero, A7 UART1 TX is
one, A8 UART1 RX is one, A9/A14 SARADC channel encodings are five, B7 SPI0
SSN is one, B8 SPI0 CLK is one, B10 SPI0 MOSI is one, B11 SWDIO is one,
B14 SWCLK is one, and the GPIO encodings cleared in these operations are
zero.

## Ordered write contract

The plan records each C compound assignment as its own write. `value` is the
register value after that operation, computed from the preceding value for
that register.

| Order | Register | Operation | Operand |
| --- | --- | --- | --- |
| 1 | PORTA SEL0 | Clear | A3–A6 mask `0x0ffff000` |
| 2 | PORTA SEL0 | Set | A7 UART1 TX `0x10000000` |
| 3 | PORTA SEL1 | Clear | A10–A13 mask `0x00ffff00` |
| 4 | PORTA SEL1 | Set | A8 UART1 RX, A9 SARADC CH4, A14 SARADC CH9 `0x05000051` |
| 5 | PORTB SEL0 | Clear | B7 mask `0xf0000000` |
| 6 | PORTB SEL0 | Set | B7 SPI0 SSN `0x10000000` |
| 7 | PORTB SEL1 | Clear | B9, B11, B14, B15 mask `0xff00f0f0` |
| 8 | PORTB SEL1 | Set | B8 SPI0 CLK and B10 SPI0 MOSI (`0x00000101`); with `ENABLE_SWD`, also B11 SWDIO and B14 SWCLK (`0x01001101` total) |
| 9 | PORTC SEL0 | Clear | C0–C5 mask `0x00ffffff` |

The zero-valued GPIO selections still contribute to the source clear masks;
the set operations remain separate even when their set bits are zero or the
resulting register value is unchanged. In particular, PORTA SEL0 A7 and
PORTA SEL1 A8, A9, and A14 are ORed without first being cleared. PORTB SEL1
B14 and B15 are cleared in both SWD variants; the non-SWD branch does not set
B14 again. These details preserve the source write trace rather than
normalizing it to a final pin configuration.

## Scope and validation

This is a fixed-size, allocation-free software trace over caller-supplied
register snapshots. It does not configure input enable or pulls, perform
MMIO, write pins, visit devices, or change startup, transport, ABI, or debug
access procedures. It is not evidence of hardware behavior.

Focused checks from the repository root:

```sh
cargo test --locked --test portcon_plan
cargo clippy --locked --test portcon_plan -- -D warnings
rustfmt --check --edition 2024 src/portcon_plan.rs tests/portcon_plan.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/portcon_plan.rs -o /tmp/portcon_plan.rlib
```

The host tests check literal ordered values from varied and zero snapshots,
including both `ENABLE_SWD` branches. The standalone target compile checks
that this module needs neither `std` nor an allocator; it does not link the
plan into firmware startup.
