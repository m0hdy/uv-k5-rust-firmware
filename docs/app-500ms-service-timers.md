# Offline FM and backlight 500 ms timer decisions

[`src/app_500ms_service_timers.rs`](../src/app_500ms_service_timers.rs) models
the FM countdown and backlight-timeout prefix of pinned
[`app/app.c::APP_TimeSlice500ms`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/app/app.c#L1315-L1328)
from commit `7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`. The counter widths
follow `app/fm.c` and `driver/backlight.c`/`.h`; the always-on option index is
7 from the eight-entry `ui/menu.c` backlight table.

When FM support is enabled, a positive FM countdown is decremented first. If FM
mode is active, this slice returns immediately even when that decrement reached
zero, so no backlight countdown or off request occurs in the same call.
Otherwise, a positive backlight countdown decrements only when the save prompt,
CSS background scan and selected ABR/ABR_MAX menu do not hold it. Expiry returns
a backlight-off request unless the setting is the `ON` option. The caller owns
all state, feature context and any physical backlight effect.

This code does not schedule the slice, measure elapsed time, access MMIO or
close parent issue #52.

## Focused validation

```sh
cargo test --locked --test app_500ms_service_timers
cargo clippy --locked --test app_500ms_service_timers -- -D warnings
rustfmt --edition 2024 --check src/app_500ms_service_timers.rs tests/app_500ms_service_timers.rs
RUSTFLAGS="-D warnings" cargo check --locked --lib --target thumbv6m-none-eabi
```

Host cases cover FM feature/mode, early return and zero boundaries; each
backlight hold condition, ABR/ABR_MAX exceptions, the always-on option,
simultaneous deadlines, and maximum-width counters.
