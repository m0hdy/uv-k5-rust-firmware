# Offline scheduler countdown state

## Pinned behavior

[`scheduler_countdowns`](../src/scheduler_countdowns.rs) models the ordered
countdown and pending-flag mutations in
[`scheduler.c:65–112`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/scheduler.c#L65-L112)
from `egzumer/uv-k5-firmware-custom` at commit
`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`.

The C `DECREMENT` and `DECREMENT_AND_TRIGGER` definitions are in
[`scheduler.c:31–41`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/scheduler.c#L31-L41).
Counter and flag declarations are in
[`misc.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/misc.h),
[`helper/battery.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/helper/battery.h),
[`audio.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/audio.h), and
[`app/fm.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/app/fm.h).
Function, scan direction, dual-watch, and FM scan predicates follow
[`functions.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/functions.h),
[`misc.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/misc.h),
[`app/chFrScanner.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/app/chFrScanner.h),
[`settings.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/settings.h), and
[`app/fm.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/app/fm.h).

The C `DECREMENT` macro decrements only a positive counter. The
`DECREMENT_AND_TRIGGER` macro also acts only on a positive counter, sets its
flag only when that decrement reaches zero, and never clears a pending flag.
The model retains the declared counter widths: the found-CDCSS and found-CTCSS
and boot counters are `u8`; the other modeled counters are `u16`. It also keeps
the feature-gated NOAA countdown separate from the NOAA-mode scheduling
countdown and flag.

Each call receives feature presence, function, scan direction, CSS-background
scan state, dual-watch state, NOAA mode, and FM scan state explicitly. The
function and scan enums describe the pinned C names and predicates; they do not
assign or assume C enum representations. No production defaults are inferred.
The executor is exported from the library and is called by the target SysTick
handler through the interrupt-safe scheduler bridge. Its countdown mutations
operate on RAM state only and do not perform peripheral work.

## Focused validation

Run from the repository root:

```sh
cargo test --locked --test scheduler_countdowns
cargo clippy --locked --test scheduler_countdowns -- -D warnings
cargo fmt --all -- --check
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi -D warnings \
  src/scheduler_countdowns.rs -o /tmp/lib_scheduler_countdowns.rlib
```

The host tests cover zero, one, multiple and maximum counter values, saturation,
sticky flags, each feature gate, all scheduler function predicates, both scan
directions, CSS-background scan, the separate NOAA countdown branches, and
simultaneous countdown state changes. The direct Cortex-M0 compilation checks
this standalone `no_std` module with warnings denied. These are pure state
checks; they do not establish interrupt installation, real-time behavior,
application integration, RF behavior, or device operation. Issue
[#52](https://github.com/m0hdy/uv-k5-rust-firmware/issues/52) remains open for
those concrete behaviors.
