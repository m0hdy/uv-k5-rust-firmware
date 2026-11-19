# Display settings startup defaults provenance

[`src/display_boot_defaults.rs`](../src/display_boot_defaults.rs) models only
the eight bytes at EEPROM address `0x0E78` in
[`SETTINGS_InitEEPROM()`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/settings.c#L59-L72)
from `egzumer/uv-k5-firmware-custom` commit
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/commit/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).

The backlight submenu length is eight, from the `[8][7]` declaration of
`gSubMenu_BACKLIGHT` in pinned [`ui/menu.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/ui/menu.h)
and its eight entries in pinned [`ui/menu.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/ui/menu.c).
`MDF_FREQUENCY` is 0, `CROSS_BAND_OFF` is 0, and `DUAL_WATCH_CHAN_A` is 1
in pinned [`settings.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/settings.h).
`BLMIN_STAT_ON` is the first `BLMIN_STAT_t` value (0) in pinned
[`driver/backlight.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/backlight.h).

The enum values used by the decoder are `MDF_FREQUENCY`/`MDF_CHANNEL`/
`MDF_NAME`/`MDF_NAME_FREQ` = `0/1/2/3`, `CROSS_BAND_OFF`/
`CROSS_BAND_CHAN_A`/`CROSS_BAND_CHAN_B` = `0/1/2`, and
`DUAL_WATCH_OFF`/`DUAL_WATCH_CHAN_A`/`DUAL_WATCH_CHAN_B` = `0/1/2`.
Backlight time indices `0..=7` name, in order, `OFF`, `5 sec`, `10 sec`,
`20 sec`, `1 min`, `2 min`, `4 min`, and `ON`.

| Byte | Field | Accepted values | Invalid value fallback |
| --- | --- | --- | --- |
| 0 low nibble | Backlight max | `0..=10` | `10` |
| 0 high nibble | Backlight min | Less than the decoded max | `0` |
| 1 | Channel display mode | `0..=3` | `MDF_FREQUENCY` (`0`) |
| 2 | Cross-band RX/TX | `0..=2` | `CROSS_BAND_OFF` (`0`) |
| 3 | Battery save | `0..=4` | `4` |
| 4 | Dual watch | `0..=2` | `DUAL_WATCH_CHAN_A` (`1`) |
| 5 | Backlight time | `0..=7` | `3` |
| 6 | Tail-tone elimination | `0..=1` | `false` |
| 7 | VFO open | `0..=1` | `true` |

The max nibble is clamped before the min nibble is checked, and min must be
strictly less than that resulting max. With `ENABLE_BLMIN_TMP_OFF`, startup
assigns `BLMIN_STAT_ON` independently of EEPROM data; without the feature,
`backlight_min_status` is `None`. That `None` represents an omitted field
assignment, not an EEPROM reset or a write of any kind.

The host tests use literal inputs and expected results for valid bounds,
each field's first invalid value, erased bytes, equal and greater min/max, and
both `ENABLE_BLMIN_TMP_OFF` states. This is a pure software model. It does not
access EEPROM, change programming or RF behavior, integrate with startup, or
claim physical parity.

Focused validation from the repository root:

```sh
cargo test --locked --test display_boot_defaults
cargo clippy --locked --test display_boot_defaults -- -D warnings
rustfmt --check --edition 2024 src/display_boot_defaults.rs tests/display_boot_defaults.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/display_boot_defaults.rs -o /tmp/display_boot_defaults.rlib
```

The direct target compile verifies standalone `no_std` Cortex-M0 compilation;
it does not link the decoder into firmware startup.
