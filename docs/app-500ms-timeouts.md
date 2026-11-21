# Offline application 500 ms timeout transitions

[`src/app_500ms_timeouts.rs`](../src/app_500ms_timeouts.rs) models the first
pure timeout mutations in pinned
[`app/app.c::APP_TimeSlice500ms`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/app/app.c#L1266-L1304)
from commit `7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`. The counter widths follow declarations in `misc.c`/`misc.h`; the center-line
discriminants follow
`ui/main.h`.

The caller owns the state and supplies whether RSSI-bar is enabled, the current
center-line, and whether the menu is selected. The transition decrements the
8-bit keypad-lock counter and marks the display dirty at zero; decrements the
8-bit live DTMF timeout only when its optional center-line gate allows it,
clears all 20 bytes only when that timeout expires with nonempty text, and marks
the display dirty; then decrements the 16-bit menu timeout and returns `exit_menu`
only when it expires on the menu screen. A preexisting display-dirty flag stays
set.

This is a pure 500 ms slice prefix. It does not schedule elapsed time, call the
UI/audio/storage drivers, handle the later `APP_TimeSlice500ms` operations, or
wire an application loop or interrupt.

## Focused validation

```sh
cargo test --locked --test app_500ms_timeouts
cargo clippy --locked --test app_500ms_timeouts -- -D warnings
rustfmt --edition 2024 --check src/app_500ms_timeouts.rs tests/app_500ms_timeouts.rs
RUSTFLAGS="-D warnings" cargo check --locked --lib --target thumbv6m-none-eabi
```

The host cases cover zero/expiry/max counters, each RSSI center-line state,
nonempty and empty DTMF text, sticky display updates, menu gating, and
simultaneous expiries. This does not close parent issue #52; operational
application and device evidence remain outstanding.
