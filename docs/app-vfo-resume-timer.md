# Offline VFO resume timer

[`src/app_vfo_resume_timer.rs`](../src/app_vfo_resume_timer.rs) models the
VFO-resume countdown in pinned
[`app/app.c::APP_TimeSlice500ms`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/app/app.c#L1443-L1450)
from commit `7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`. The countdown is an
8-bit value (`misc.c::gVFOStateResumeCountdown_500ms`). PTT holds the value;
otherwise a positive value decrements. Expiry requests normal VFO state. With
FM enabled, FM mode active and the receiver idle, expiry also requests FM
restart and FM display selection.

The caller retains ownership of radio state, FM startup and display effects.
This module does not schedule the 500 ms slice or access hardware.

## Focused validation

```sh
cargo test --locked --test app_vfo_resume_timer
cargo clippy --locked --test app_vfo_resume_timer -- -D warnings
rustfmt --edition 2024 --check src/app_vfo_resume_timer.rs tests/app_vfo_resume_timer.rs
RUSTFLAGS="-D warnings" cargo check --locked --lib --target thumbv6m-none-eabi
```
