# Offline memory-channel numeric input flow

`src/app_numeric_input_flow.rs` models the memory-channel branch of
[`app/main.c:263–322`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/app/main.c#L263-L322)
at pinned revision
`egzumer/uv-k5-firmware-custom@7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`.
It re-exports `Key` and `KeyEvent` from `keyboard_poll_flow` and composes those
shared types with `NumericInputBox`. It returns ordered `FlowEffect` values
describing the processed trace; the function mutates the input box and runs
the injected channel-validity query before returning those effects. The query
models the pinned
[`RADIO_CheckValidChannel` guard in `radio.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/radio.c#L59-L80),
which rejects values outside the memory-channel range (0–199). The separate
`u8` conversion guard rejects a positive query result that still cannot fit in
`u8`, rather than narrowing it and potentially selecting a different channel.

A digit press requests the optional key beep but does not append. Only a
non-held digit release without F modifies the input box, refreshes the timeout,
and requests the main display. The C branch assigns `DISPLAY_MAIN` both after
the append and again for a partial memory-channel entry; these identical
assignments coalesce into the single idempotent `RequestMainDisplay` effect.
Once three digits are present, the index resets before validity is queried.
Invalid channels retain the optional double-beep request. Valid channels
update the selected VFO memory and screen channel, request a VFO save, then
request reload, in that order. The caller owns mode eligibility; held/F
shortcut actions, frequency and NOAA modes, actual persistence, and rendering
are outside this model.

The C expression subtracts one from an unsigned channel calculation. For
`000`, C's conversion yields the `uint16_t` result `65535`. Rust makes that
behavior explicit with `u16::wrapping_sub(1)`, also yielding `u16::MAX`; the
validity query rejects it for the pinned memory-channel range.

Run focused checks from the repository root:

```sh
cargo test --locked --test app_numeric_input_flow
cargo clippy --locked --test app_numeric_input_flow -- -D warnings
rustfmt --check --edition 2024 \
  src/app_numeric_input_flow.rs tests/app_numeric_input_flow.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/app_numeric_input_flow.rs \
  -o /tmp/app_numeric_input_flow.rlib
```

The host tests cover literal press/release traces, leading zeros, partial and
subsequent entry, `000`, lower/upper and invalid candidates, held/F-modified/
non-digit exclusion, and selected-VFO forwarding.
