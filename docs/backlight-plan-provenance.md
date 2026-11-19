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
source write. The integer source expression is
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

`brightness_compare_value` models `(1 << brightness) - 1` for the supported
settings range 0–10, returning compare values 0 through 1023. Values outside
that range return `BrightnessOutOfRange` before any shift is evaluated, rather
than reproducing undefined C shift behavior.

## Scope and validation

The module does not configure timing policy, countdown state, board wiring,
EEPROM settings, or hardware, and neither its trace nor compare values
establish physical backlight output.

Focused checks from the repository root:

```sh
cargo test --locked --test backlight_plan
cargo clippy --locked --test backlight_plan -- -D warnings
rustfmt --check --edition 2024 src/backlight_plan.rs tests/backlight_plan.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/backlight_plan.rs -o /tmp/backlight_plan.rlib
```

The host tests check exact ordered operations and values across distinct clock
and mux snapshots, plus every supported brightness value and out-of-range
inputs. Standalone Cortex-M0 compilation verifies that the plan needs neither
`std` nor an allocator; it does not link the module into firmware startup.
