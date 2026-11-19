# Offline numeric input box

[`src/numeric_input_box.rs`](../src/numeric_input_box.rs) models
`INPUTBOX_Append()` and `INPUTBOX_GetAscii()` from
[`ui/inputbox.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/ui/inputbox.c#L21-L44)
at `egzumer/uv-k5-firmware-custom@7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`.
The numeric event values follow
[`driver/keyboard.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/keyboard.h#L23-L44):
`KEY_0` through `KEY_9` are consecutive values 0 through 9. Rust callers
provide a typed `Digit` or the explicit `InputEvent::NoDigit`; no raw key
codes are accepted.

The state has eight slots. Its initial zero-initialized cells project as
`00000000`. An append while the index is zero first changes all slots to dash
placeholders, including an append of `NoDigit`; a digit is then stored and
advances the index. `NoDigit` at a nonzero index leaves the cells and index
unchanged. Appends at capacity do not mutate the state. `reset_index()` only
resets the index and preserves the prior cells until a subsequent append.

`ascii_projection()` returns exactly eight bytes, mapping digits to ASCII and
placeholders to `-`. It deliberately excludes the C buffer's ninth NUL byte;
the projection is a fixed-width, non-NUL-terminated Rust value.

## Scope and validation

This is an allocation-free, standalone `no_std` state model. It has no key
dispatch, firmware wiring, MMIO, lock/password behavior, or RF flow, and does
not claim device behavior.

Focused checks from the repository root:

```sh
cargo test --locked --test numeric_input_box
cargo clippy --locked --test numeric_input_box -- -D warnings
rustfmt --check --edition 2024 src/numeric_input_box.rs tests/numeric_input_box.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/numeric_input_box.rs -o /tmp/numeric_input_box.rlib
```

The host tests cover initialization, invalid/no-digit input at the start and
mid-entry, leading zero, capacity and overflow, and index reset followed by a
shorter entry. Standalone Cortex-M0 compilation verifies target compatibility
without linking this module into firmware.
