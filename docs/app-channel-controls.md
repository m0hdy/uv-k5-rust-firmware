# Offline main-screen up/down controls

`app_channel_controls::process` models the pinned
[`MAIN_Key_UP_DOWN`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/app/main.c#L601-L682)
transition at commit `7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`. It separates
key-event policy from the frequency-step, receive-band validation, next-memory
channel search, scanner, radio, persistence, and voice operations supplied by
the eventual firmware adapter.

The same module includes the pinned frequency rounding and step-with-band-limit
helpers. These preserve the 8.33 kHz channel calculation, the legacy half-step
for steps at or above 1 kHz, unsigned addition wrap, and band-edge wraparound.

The model preserves short-press beeps, input-box rejection, long-release
channel announcement, frequency rejection before radio effects, memory-channel
no-op behavior, NOAA wraparound, and scan pause reset ordering. Effect traces
are synchronous and allocation-free.

This is host-testable policy, not a live application integration. Firmware does
not yet call this module, so it does not change target key behavior or establish
device parity. The remaining application story also requires wiring the
effects to verified radio/display/storage interfaces and comparing channel,
frequency, scan, receive, reset, and timing behavior on hardware.

## Focused validation

Run from the repository root:

```sh
cargo test --locked --test app_channel_controls
cargo clippy --locked --test app_channel_controls -- -D warnings
cargo fmt --all -- --check
```
