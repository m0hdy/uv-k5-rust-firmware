# Offline SARADC battery acquisition

`src/adc_battery_acquisition.rs` models the acquisition portion of
`BOARD_ADC_GetBatteryInfo()` from pinned
[`board.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/board.c)
and `ADC_Start()`, `ADC_CheckEndOfConversion()`, and `ADC_GetValue()` from
[`driver/adc.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/adc.c),
using the DP32G030 [`saradc.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/saradc.h)
definitions at pinned revision
[`7607f0a`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).

The adapter reads START immediately before its read-modify-write to set
`SARADC_START_START`. It polls channel 9 STAT for EOC, then reads channel 4
voltage followed by channel 9 current. As in `ADC_GetValue()`, each channel's
IF bit is acknowledged before its DATA register is read. Both EOC and sample
fields use the shared `decode_adc_sample()` decoder in `src/adc_samples.rs`.
The interface is typed and address-free; this is not an MMIO implementation
or startup wiring.

Unlike the pinned C function's unbounded `while (!ADC_CheckEndOfConversion())`
loop, this model takes a caller-supplied poll budget and returns
`PollBudgetExhausted` if channel 9 is not ready within that many status reads.
A zero budget returns without performing any I/O. This bound is a modeling
choice for offline callers, not a timing guarantee.

## Focused validation

From the repository root:

```sh
cargo test --locked --test adc_battery_acquisition
cargo clippy --locked --test adc_battery_acquisition -- -D warnings
rustfmt --check --edition 2024 \
  src/adc_battery_acquisition.rs tests/adc_battery_acquisition.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/adc_battery_acquisition.rs -o /tmp/adc_battery_acquisition.rlib
```
