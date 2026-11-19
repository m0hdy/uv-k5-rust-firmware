# Offline startup version display

[`src/startup_version_display.rs`](../src/startup_version_display.rs) builds a
fixed-size, allocation-free plan for the M0HDY startup version text. With no
version it produces `M0HDY`; with a supplied version it produces
`M0HDY <version>`. An explicitly empty version is rejected. Versions must
contain printable ASCII bytes, and text wider than the display is rejected.
The `None` and `FullScreen` display modes suppress the version text without
validating an unused version.

The placement follows the text-bearing branch of upstream
[`ui/welcome.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/ui/welcome.c#L42-L76)
at pinned revision
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
That branch draws the small-normal version at line 6 with bounds 0–128.
[`ui/helper.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/ui/helper.c#L91-L116)
centers that text using the glyph width plus one spacing column. The pinned
[`font.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/font.h#L26)
declares six columns per small-font glyph; no font data is copied here.

The renderer trait makes the planned text and placement mockable. This module
does not write to the display, read EEPROM, change startup wiring, or add
container/version padding.

Run focused checks from the repository root:

```sh
cargo test --locked --test startup_version_display
cargo clippy --locked --test startup_version_display -- -D warnings
cargo fmt --all -- --check
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/startup_version_display.rs -o /tmp/startup_version_display.rlib
```
