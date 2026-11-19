# Default boot device clock gate assignment

`src/device_clock_gate_startup.rs` models the unconditional `DeviceClockGates`
step in the default boot sequence. It is isolated from `src/lib.rs`, the board
sequencer, and firmware startup. It supplies a typed, write-only I/O boundary
and an ARM bare-metal volatile backend; it does not call either from startup.

## Pinned assignment

At
[`egzumer/uv-k5-firmware-custom@7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3),
[`main.c:61–76`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/main.c#L61-L76)
assigns the full `SYSCON_DEV_CLK_GATE` register before `SYSTICK_Init()` and
`BOARD_Init()`. The terms are unconditional, including UART1 and CRC even if
their later initialization is conditional. The matching
[`syscon.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/syscon.h#L25-L25)
sets the SYSCON base to `0x40000000`, and its
[device gate definitions](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/syscon.h#L118-L318)
set the offset to `0x8` and encode each enabled field as one bit:

| Enabled field | Bit |
| --- | ---: |
| GPIOA, GPIOB, GPIOC | 0, 1, 2 |
| UART1 | 7 |
| SPI0 | 10 |
| PWM_PLUS0 | 20 |
| SARADC | 25 |
| CRC | 27 |
| AES | 28 |

The resulting single 32-bit write is `0x1a100487` to `0x40000008`. Every
other bit is zero, irrespective of the previous register value; there is no
snapshot read or read/modify/write.

This assignment is distinct from the overlay-only
[`BOARD_FLASH_Init()` / `SYSTEM_ConfigureClocks()`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/board.c#L47-L59)
path modeled by `src/clock_plan.rs` (issue #152). It does not configure the
oscillator, clock selection, or divider gate.

## Target ownership and safety

The target entry `apply_device_clock_gates` is unsafe and available only for
ARM bare-metal targets. The caller must establish that a DP32G030 maps this
register at `0x40000008` and accepts a 32-bit write, and must own the clock
configuration and register exclusively during the operation. Because the
assignment clears all unlisted gates, affected peripherals must be idle and
safe to clock-gate, and no other context may depend on their clocks remaining
enabled. This module does not establish those conditions or validate hardware.

## Validation

The host fake implements a write-only trait, so the operation has no read
interface. Tests assert the literal address and enabled bitset, zero outside
that bitset, and one replacement write from zero and nonzero initial values.

```sh
cargo test --locked --test device_clock_gate_startup
cargo clippy --locked --test device_clock_gate_startup -- -D warnings
rustfmt --check --edition 2024 \
  src/device_clock_gate_startup.rs tests/device_clock_gate_startup.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings -C opt-level=2 -C link-dead-code=yes --emit=asm \
  src/device_clock_gate_startup.rs -o /tmp/device_clock_gate_startup.s
```

With the repository toolchain, inspection of `apply_device_clock_gates` in the
emitted Cortex-M0 assembly shows literal-pool loads of `1073741832`
(`0x40000008`) and `437257351` (`0x1a100487`), followed by one word
`str` to that address. There is no load from the register. The function's
stack save is not an MMIO write. `link-dead-code` retains the otherwise unused
entry for inspection; the assembly is not a firmware or release artifact.
These checks do not establish device behavior.
