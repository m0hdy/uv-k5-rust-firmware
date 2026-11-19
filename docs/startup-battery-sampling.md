# Offline startup battery sampling

`src/startup_battery_sampling.rs` composes the bounded acquisition in
[`adc_battery_acquisition.rs`](../src/adc_battery_acquisition.rs) and
`calibrated_battery_voltage` in [`adc_samples.rs`](../src/adc_samples.rs).
It models the four-read loop immediately before `BATTERY_GetReadings(false)` in
pinned [`main.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/main.c)
and only the averaging and calibration in
[`helper/battery.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/helper/battery.c).
The caller supplies the calibration corresponding to `gBatteryCalibration[3]`
and the per-acquisition poll budget.

On success the result contains the four voltage samples in acquisition order,
the current from the fourth acquisition, and the calibrated voltage. A zero
calibration returns `Calibration(ZeroCalibration)` before any I/O, even if the
poll budget is also zero; this avoids the pinned C division by zero. On an
acquisition error the result identifies the zero-based sample index and
underlying error, stops immediately, and returns no partial readings. I/O from
earlier acquisitions and the failing acquisition is not rolled back.

This standalone `no_std` model has no MMIO or startup wiring. It does not model
the earlier pre-settings reading, percentage curve, battery level transitions,
or charging UI.

## Focused validation

From the repository root:

```sh
cargo test --locked --test startup_battery_sampling
cargo clippy --locked --test startup_battery_sampling -- -D warnings
rustfmt --check --edition 2024 \
  src/startup_battery_sampling.rs tests/startup_battery_sampling.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/startup_battery_sampling.rs -o /tmp/startup_battery_sampling.rlib
```
