# Settings EEPROM startup-read composition

[`src/settings_boot_composition.rs`](../src/settings_boot_composition.rs)
composes the existing read planner with the core and display startup-default
decoders. The modeled sequence is limited to the first two eight-byte reads in
[`SETTINGS_InitEEPROM()`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/settings.c#L41-L72)
and [`EEPROM_ReadBuffer()`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/eeprom.c#L24-L37)
at `egzumer/uv-k5-firmware-custom` commit
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/commit/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3):

| Order | Address | Length | Decoder |
| --- | --- | --- | --- |
| 1 | `0x0E70` | 8 | `decode_core_settings` |
| 2 | `0x0E78` | 8 | `decode_display_boot_defaults` |

`ReadBackend` is the preserved infallible offline boundary: it receives each
existing `ReadPlan` in call order and returns one eight-byte buffer.
`compose_with_transport` is the fallible entry point. It executes those same
plans through the EEPROM read executor, returns the failed block together with
the original typed read error, and does not issue the display read if the core
read fails. Both entries use the same settings decoders. An erased `[0xff; 8]`
response remains ordinary data and is passed to the corresponding decoder.
No MMIO, physical transport, EEPROM write, additional settings block, or
startup wiring is involved.

The host tests check both complete command traces, the order and lengths of
the requests, routing to each decoder, all NOAA/VOX/BLMIN_TMP_OFF presence
combinations, both block-specific transport failures and cleanup, and erased
bytes in either block.

From the repository root:

```sh
cargo test --locked --test settings_boot_composition
cargo clippy --locked --test settings_boot_composition -- -D warnings
rustfmt --check --edition 2024 src/settings_boot_composition.rs tests/settings_boot_composition.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/settings_boot_composition.rs -o /tmp/settings_boot_composition.rlib
```

The direct Rust compile checks the isolated `no_std` composition for Cortex-M0;
it does not link the module into firmware startup or validate hardware behavior.
