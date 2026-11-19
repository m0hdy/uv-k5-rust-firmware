# Offline PortCon input and pull trace

[`src/portcon_input_plan.rs`](../src/portcon_input_plan.rs) models the
input-enable, pull-up, pull-down, and open-drain register assignments in
[`BOARD_PORTCON_Init()`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/board.c#L226-L455)
from `egzumer/uv-k5-firmware-custom@7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`.
Register fields and enable/disable encodings follow the matching
[DP32G030 PORTCON definitions](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h).
Each IE, PU, PD, and OD field is one bit at the corresponding pin number;
`ENABLE` is one and `DISABLE` is zero.

The planner accepts a separate caller-supplied snapshot for each of the
twelve port/register pairs. It records each compound assignment as a
distinct ordered write, including the value after the operation. Set writes
OR their literal bits into the preceding value; clear writes AND that value
with the complement of the source mask.

## Ordered assignments

| Order | Register | Operation | Bits |
| --- | --- | --- | --- |
| 1 | PORTA IE | Set | A3–A6 and A8 `0x00000178` |
| 2 | PORTA IE | Clear | A10–A13 `0x00003c00` |
| 3 | PORTA PU | Set | A3–A6 `0x00000078` |
| 4 | PORTA PU | Clear | A10–A13 `0x00003c00` |
| 5 | PORTA PD | Clear | A3–A6 and A10–A13 `0x00003c78` |
| 6 | PORTA OD | Set | A3–A6 `0x00000078` |
| 7 | PORTA OD | Clear | A10–A13 `0x00003c00` |
| 8 | PORTB IE | Set | B14 `0x00004000` |
| 9 | PORTB IE | Clear | B6–B10, B11, and B15 `0x00008fc0` without `ENABLE_SWD`; B6–B10 and B15 `0x000087c0` with it |
| 10 | PORTB PU | Clear | B6, B9, B11, B14, and B15 `0x0000ca40` |
| 11 | PORTB PD | Clear | B6, B9, B11, B14, and B15 `0x0000ca40` |
| 12 | PORTB OD | Clear | B6, B9, B11, and B15 `0x00008a40` |
| 13 | PORTB OD | Set | B14 `0x00004000` |
| 14 | PORTC IE | Set | C5 `0x00000020` |
| 15 | PORTC IE | Clear | C0–C4 `0x0000001f` |
| 16 | PORTC PU | Set | C5 `0x00000020` |
| 17 | PORTC PU | Clear | C0–C4 `0x0000001f` |
| 18 | PORTC PD | Clear | C0–C5 `0x0000003f` |
| 19 | PORTC OD | Clear | C0–C4 `0x0000001f` |
| 20 | PORTC OD | Set | C5 `0x00000020` |

`ENABLE_SWD` affects only assignment 9: the source condition excludes B11
from the IE clear mask when SWD is enabled. The other B11 pull/open-drain
clears and the B14 open-drain set are unconditional. Assignment 20 preserves
the source OR operation: the disabled C0–C4 fields contribute zero bits and
C5 contributes the enabled bit.

## Scope and limitations

This is an allocation-free, offline transformation of supplied integers.
It does not access memory-mapped registers, read device state, configure
hardware, or validate electrical behavior. The snapshots are test inputs,
not captured hardware values; the traces establish neither a board
configuration nor behavior on any physical radio or board revision.
No hardware debug operation or device test is part of this model.

Focused validation from the repository root:

```sh
cargo test --locked --test portcon_input_plan
cargo clippy --locked --test portcon_input_plan -- -D warnings
rustfmt --check --edition 2024 src/portcon_input_plan.rs tests/portcon_input_plan.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/portcon_input_plan.rs -o /tmp/portcon_input_plan.rlib
```
