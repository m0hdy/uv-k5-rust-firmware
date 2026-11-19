# Offline backlight PWM plan

`src/backlight_plan.rs` models the six ordered writes in
[`BACKLIGHT_InitHardware()`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/backlight.c#L28-L53)
and the compare arithmetic in
[`BACKLIGHT_SetBrightness()`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/backlight.c#L108-L113)
from `egzumer/uv-k5-firmware-custom@7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`.
The B6 mux mask and PWMP0_CH0 field value follow the pinned
[`portcon.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h#L346-L352);
generator and configuration bits follow
[`pwmplus.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/pwmplus.h#L14-L41).

## Initialization write trace

`BacklightInitPlan::from_snapshot` takes the initial PWM_PLUS0 clock-source
register and PORTB SEL0 value and records the post-operation value for each
source write as an offline prediction. The integer source expression is
`(48_000_000 / 1024 / 1000) << 16`, so the OR operand is `0x002e0000`.
This preserves the C integer divisions and clock-register OR; it does not
derive a PWM frequency from the stale nearby comment or establish physical
PWM output.

| Order | Register | Operation | Operand / assigned value |
| ---: | --- | --- | --- |
| 1 | PWM_PLUS0_CLKSRC | OR | `0x002e0000` |
| 2 | PWM_PLUS0_PERIOD | Assign | `1023` |
| 3 | PORTB_SEL0 | Clear | B6 mask `0x0f000000` |
| 4 | PORTB_SEL0 | Set | B6 PWMP0_CH0 bits `0x01000000` |
| 5 | PWM_PLUS0_GEN | Assign | `0x01010000` |
| 6 | PWM_PLUS0_CFG | Assign | `0x00000005` |

The pin-mux clear and set are separate writes in the original order. All
register values affected by the initial snapshot are calculated from that
snapshot; fixed assignments are recorded literally. The trace is fixed-size
and allocation-free and performs no MMIO.

`src/backlight_startup_executor.rs` applies the same ordered operations
through typed `RegisterIo`. It uses the plan only for operation order,
registers, operands, and assignment values: each OR, clear, and set reads the
register immediately before its write. In particular, the B6 set performs a
second PORTB SEL0 read after the clear write. Assignments issue no reads. Its
target-only entry maps the same typed registers to aligned 32-bit volatile
accesses:

| Register | Address |
| --- | ---: |
| PWM_PLUS0_CLKSRC | `0x400b4008` |
| PWM_PLUS0_PERIOD | `0x400b401c` |
| PORTB_SEL0 | `0x400b0008` |
| PWM_PLUS0_GEN | `0x400b4004` |
| PWM_PLUS0_CFG | `0x400b4000` |

The PWM_PLUS0 addresses follow the pinned
[`pwmplus.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/pwmplus.h);
PORTB SEL0 follows the pinned
[`portcon.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/portcon.h).

The unsafe entry requires the PWM_PLUS0 clock gate to be enabled, B6 to be
reserved for the backlight, and exclusive access to these registers for the
whole sequence. It does not configure clocks or pin ownership, change
brightness, or connect to firmware startup.

`brightness_compare_value` models `(1 << brightness) - 1` for the supported
settings range 0–10, returning compare values 0 through 1023. Values outside
that range return `BrightnessOutOfRange` before any shift is evaluated, rather
than reproducing undefined C shift behavior.

## Scope and validation

`src/backlight_plan.rs` does not configure timing policy, countdown state,
board wiring, EEPROM settings, or hardware. Neither its trace nor compare
values establish physical backlight output.

Focused checks from the repository root:

```sh
cargo test --locked --test backlight_plan
cargo test --locked --test backlight_startup_executor
cargo test --locked --test backlight_startup_adapter
cargo clippy --locked --test backlight_plan -- -D warnings
cargo clippy --locked --test backlight_startup_executor -- -D warnings
cargo clippy --locked --test backlight_startup_adapter -- -D warnings
rustfmt --check --edition 2024 src/backlight_plan.rs src/backlight_startup_executor.rs \
  tests/backlight_plan.rs tests/backlight_startup_executor.rs \
  tests/backlight_startup_adapter.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/backlight_plan.rs -o /tmp/backlight_plan.rlib
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi -D warnings \
  -C opt-level=2 -C link-dead-code=yes --emit=asm \
  src/backlight_startup_executor.rs -o /tmp/backlight_startup_executor.s
```

The host tests check exact ordered operations and values across distinct clock
and mux snapshots, the live executor's chronological reads and writes
(including changing PORTB values), assignments without reads, the target
address map and alignment, and every supported brightness value and
out-of-range input. Standalone Cortex-M0 compilation verifies that both
modules need neither `std` nor an allocator. Compile the target adapter with
assembly emission and inspect `initialize_backlight_hardware` for 32-bit
volatile loads and stores in the mapped register order; neither compilation
nor inspection executes the code or links it into firmware startup.
