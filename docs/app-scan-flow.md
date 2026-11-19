# Offline scheduled scan continuation

`src/app_scan_flow.rs` composes the existing `ScanState` and
`ScanEvent::Continue` model with a typed `ScanEffects` boundary. The behavior
is based on the pinned
[`app.c:818–837`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/app/app.c#L818-L837)
and
[`chFrScanner.c:70–90`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/app/chFrScanner.c#L70-L90)
at commit `7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`.

The flow stores the scan kind at start and uses it for each advance effect.
An active scheduled continuation is held when reduced service is active, an
active FM countdown is pending, the background scanner is running, PTT is
pressed, or voice support is enabled with queued voice data. When unblocked,
the existing state machine determines whether to advance or listen. Frequency
scans listen whenever the current function is incoming; memory scans listen
only for incoming with code type off. The listen effect independently selects
monitor or receive.

The C continuation performs `APP_StartListening` or the channel/frequency
advance before clearing the pause, reception, and schedule flags. The Rust flow
applies `Continue` to a copy of the state, emits its single effect, and commits
that resulting state afterward. Thus a blocked continuation preserves the
scheduled flag, and an effect backend observes the continuation before the
flow consumes it. This models synchronous effect ordering only; it does not
perform RF, device, or transmit operations and is not wired into firmware.
The broader scan-model and hardware work in issue
[#52](https://github.com/m0hdy/uv-k5-rust-firmware/issues/52) remains open.

## Focused validation

Run from the repository root:

```sh
cargo test --locked --test app_scan_flow
cargo clippy --locked --test app_scan_flow -- -D warnings
rustfmt --check --edition 2024 src/app_scan_flow.rs tests/app_scan_flow.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/app_scan_flow.rs -o /tmp/app_scan_flow.rlib
```

The host tests cover scheduled and unscheduled states, each application blocker
and resumption, both scan kinds, frequency and memory incoming behavior,
monitor/receive selection, and once-only schedule consumption.
