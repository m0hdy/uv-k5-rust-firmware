# Offline periodic scheduler and application dispatch

## Pinned behavior

`src/app_scheduler_composition.rs` composes the existing periodic flag model
from [`scheduler.c:45–64`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/scheduler.c#L45-L64)
with one-iteration dispatch from
[`main.c:222–233`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/main.c#L222-L233)
and the timeslice flag acknowledgements in
[`app.c:1088–1090`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/app/app.c#L1088-L1090) and
[`app.c:1259–1261`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/app/app.c#L1259-L1261).
The public reference is `egzumer/uv-k5-firmware-custom` at commit
`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`.

The composition delegates tick progression and sticky flag state to
`PeriodicSchedulerFlags`, and delegates the ordering of update, 10 ms and
500 ms queries to `app_timeslice_dispatch`. It acknowledges each flag at entry
to its corresponding timeslice, before invoking the typed application effect.
The 40 ms flag remains pending for a separate caller; it is not queried or
dispatched by this main-loop iteration.

The tick callback passed to the 10 ms effect lets a host mock model an
abstract tick during that effect. It only demonstrates sequential flag
visibility and dispatch order: it is not ISR synchronization or timing
evidence. This offline model adds no ISR, MMIO, startup wiring, countdown
effects, RF behavior, or device tests. The parent
[#52](https://github.com/m0hdy/uv-k5-rust-firmware/issues/52) remains open.

## Focused validation

Run from the repository root:

```sh
cargo test --locked --test app_scheduler_composition
cargo clippy --locked --test app_scheduler_composition -- -D warnings
rustfmt --check --edition 2024 \
  src/app_scheduler_composition.rs tests/app_scheduler_composition.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/app_scheduler_composition.rs \
  -o /tmp/app_scheduler_composition.rlib
```

The host tests cover idle update-only behavior, one pending tick and no
redispatch on the next iteration, the 500 ms boundary at tick 50 and the
shared 40 ms / 500 ms boundary at tick 100,
coalescing of unserviced ticks, preservation of 40 ms, and a tick delivered by
the 10 ms mock effect becoming visible to the subsequent 500 ms query without
clearing its newly pending 10 ms flag.
