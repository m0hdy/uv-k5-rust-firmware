# VFO channel-selection EEPROM decoder

[`src/channel_selection_boot_defaults.rs`](../src/channel_selection_boot_defaults.rs)
decodes the caller-provided eight-byte block at `0x0E80..0x0E87` assigned by
`SETTINGS_InitEEPROM()` in
[`egzumer/uv-k5-firmware-custom/settings.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/settings.c#L74-L85),
at pinned commit
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/commit/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
Bytes 0/1/2 map to VFO A screen/memory/frequency selections; bytes 3/4/5
map to VFO B in the same order. Each decoded channel remains an eight-bit
value, matching the C `uint8_t` fields.

The raw channel predicates are the macros in pinned
[`misc.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/misc.h#L39-L52):
`IS_VALID_CHANNEL(x)` is `x < LAST_CHANNEL`, so its accepted raw-number
interval is `0..=216`; it does not inspect EEPROM channel attributes or scan
policy. The same declarations give memory `0..=199`, frequency `200..=206`,
and NOAA `207..=216`. In pinned
[`frequencies.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/frequencies.h#L29-L37),
`BAND6_400MHz` is enum value `5`. Thus invalid screen/frequency values
default to `FREQ_CHANNEL_FIRST + BAND6_400MHz`, or `200 + 5 = 205`;
invalid memory and NOAA values default to `0` and `207`.

When `ENABLE_NOAA` is present, bytes 6/7 decode independently to NOAA
selections, defaulting invalid values to `207`. When it is absent, the result
is `None`: those bytes are not assigned another runtime field by this
decoder.

This is a raw-data decoder only. It accepts a caller-owned buffer and does not
perform EEPROM acquisition, boot composition, hardware access, or startup
wiring. Its tests use literal expectations derived from the pinned C
predicates and defaults; they do not establish device or hardware behavior.

From the repository root:

```sh
cargo test --locked --test channel_selection_boot_defaults
cargo clippy --locked --test channel_selection_boot_defaults -- -D warnings
cargo fmt --all -- --check
rustc --edition 2024 --crate-type lib --target thumbv6m-none-eabi \
  -D warnings src/channel_selection_boot_defaults.rs --emit metadata \
  -o /tmp/channel_selection_boot_defaults.rmeta
```

The direct target compile validates this standalone file's warning-denied
`no_std` compilation; the module is not exported from `src/lib.rs` and is not
connected to firmware startup.
