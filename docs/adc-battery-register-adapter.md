# DP32G030 battery SARADC register adapter

[`src/adc_battery_register_adapter.rs`](../src/adc_battery_register_adapter.rs)
maps the bounded acquisition in
[`adc_battery_acquisition.rs`](../src/adc_battery_acquisition.rs) to the
DP32G030 register addresses. The mapping follows `ADC_Start()`,
`ADC_CheckEndOfConversion()`, and `ADC_GetValue()` in pinned
[`driver/adc.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/adc.c)
and `SARADC_BASE_ADDR`, `ADC_Channel_t`, and the channel fields in
[`bsp/dp32g030/saradc.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/saradc.h)
at revision
[`7607f0a`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).

SARADC starts at `0x400ba000`. START is at offset `0x04`, IF at `0x0c`,
and channel 0 starts at `0x10`. Each `ADC_Channel_t` has two `u32` fields
(STAT then DATA), so channels have an 8-byte stride. The adapter's allowed
accesses are:

| Register | Address | Access |
| --- | ---: | --- |
| CH9 STAT | `0x400ba058` | Read |
| CH4 DATA | `0x400ba034` | Read |
| CH9 DATA | `0x400ba05c` | Read |
| START | `0x400ba004` | Read and write |
| IF | `0x400ba00c` | Write |

The acquisition interface uses separate `ReadRegister` and `WriteRegister`
types. IF cannot be selected for reads, and STAT/DATA cannot be selected for
writes. The host test checks the exact mapping and 32-bit alignment; only the
target-gated backend performs volatile accesses.

The sole target entry point is
`acquire_battery_samples_mmio(poll_budget)`. Its caller must ensure the DP32G030
mapping, an enabled SARADC clock, and a configured ADC using channel memory for
channels 4 and 9 with software triggering. The caller must also have exclusive
access to START, CH9 STAT, CH4/CH9 DATA, and IF throughout the call. Polling is
already bounded by the supplied budget; this adapter sets no separate timeout
or production `EXTTRIG_SEL` policy. It is not connected to `src/lib.rs`, boot,
or startup configuration.

## Focused validation

From the repository root:

```sh
cargo test --locked --test adc_battery_register_adapter
cargo test --locked --test adc_battery_acquisition
cargo test --locked --test startup_battery_sampling
cargo clippy --locked --test adc_battery_register_adapter -- -D warnings
rustfmt --check --edition 2024 \
  src/adc_battery_register_adapter.rs tests/adc_battery_register_adapter.rs \
  src/adc_battery_acquisition.rs tests/adc_battery_acquisition.rs \
  src/startup_battery_sampling.rs tests/startup_battery_sampling.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/adc_battery_register_adapter.rs -o /tmp/adc_battery_register_adapter.rlib
```
