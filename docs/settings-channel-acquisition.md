# EEPROM settings and channel-selection acquisition

[`src/settings_channel_acquisition.rs`](../src/settings_channel_acquisition.rs)
composes the two existing fallible startup reads with the existing decoder for
the VFO channel-selection block. It first calls
`settings_boot_composition::compose_with_transport` for `0x0e70` and `0x0e78`.
Only after both reads succeed does it execute
`plan_read(0x0e80, 8)` through the same borrowed `I2cTransport`, then decode
the result with the same `Features::noaa_enabled` flag.

`acquire_with_transport` returns the existing `SettingsBoot` together with
`ChannelSelectionBootDefaults`. An earlier read error is preserved unchanged
inside `SettingsChannelAcquisitionError::SettingsBoot`; the third read has its
own `ChannelSelection(ReadError<E>)` variant. The shared
`execute_read` retains its existing transaction behavior: a failed initial
START is not followed by STOP; an intermediate failure triggers one best-effort
STOP without replacing the original error; a failed final STOP is not retried.
No later block is attempted after failure.

The channel decoder is included by path and is not changed or exported from
`src/lib.rs`. The module is not wired into the firmware entry point. Target
users can pass the existing `eeprom_i2c_bridge::I2cBitbangTransport`; the
standalone target harness below checks that concrete bridge and the actual
target GPIO constructor compile together. Callers remain responsible for the
existing GPIOA/PortCon setup, exclusive PA10/PA11 ownership, serialization,
and delay requirements documented in
[`docs/eeprom-i2c-bridge.md`](eeprom-i2c-bridge.md). This bounded acquisition
does not cover later `SETTINGS_InitEEPROM()` blocks or establish hardware,
device, RF, or boot acceptance.

From the repository root, with the `thumbv6m-none-eabi` target installed:

```sh
cargo test --locked --test settings_channel_acquisition
cargo clippy --locked --test settings_channel_acquisition -- -D warnings
rustfmt --check --edition 2024 \
  src/settings_channel_acquisition.rs \
  tests/settings_channel_acquisition.rs \
  tests/fixtures/settings_channel_acquisition_target.rs

RUSTFLAGS="-D warnings" cargo build --locked --lib --target thumbv6m-none-eabi
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi -D warnings \
  --extern "uv_k5_rust_firmware=$(pwd)/target/thumbv6m-none-eabi/debug/libuv_k5_rust_firmware.rlib" \
  tests/fixtures/settings_channel_acquisition_target.rs \
  -o /tmp/libsettings_channel_acquisition_target.rlib
```

The host tests cover ordered transactions, all feature combinations, valid,
zero, erased and boundary bytes, every transport failure position, cleanup
precedence, write NACKs, and the existing bit-banged bridge over mock GPIO
registers. The target compile is non-test `no_std` type checking; neither check
executes hardware or proves EEPROM behavior.
