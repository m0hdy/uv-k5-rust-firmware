# Offline main-loop timeslice dispatch

## Pinned provenance

This independently authored model follows one iteration of
[`main.c:222–233`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/main.c#L222-L233)
in public `egzumer/uv-k5-firmware-custom` at commit
`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`.

## Dispatch contract

[`scheduler::dispatch`](../src/scheduler/dispatch.rs) defines a typed `Backend` and a synchronous,
allocation-free `run` for one iteration:

1. Call `update` (`APP_Update`).
2. Query `is_10ms_pending` (`gNextTimeslice`). If false, stop.
3. Call `timeslice_10ms` (`APP_TimeSlice10ms`).
4. Query the current `is_500ms_pending` (`gNextTimeslice_500ms`). If true,
   call `timeslice_500ms` (`APP_TimeSlice500ms`).

The queries occur at their respective positions, not as an upfront snapshot:
the update can change either flag, and the 10 ms hook can change the 500 ms
flag before it is read. The backend owns flag state and any effects. The
dispatcher neither clears flags nor defines elapsed-time policy.

## Focused validation

From the repository root, with the pinned Rust toolchain:

```sh
cargo test --locked --test app_timeslice_dispatch
cargo clippy --locked --test app_timeslice_dispatch -- -D warnings
cargo fmt --all -- --check
RUSTFLAGS="-D warnings" cargo check --locked --lib --target thumbv6m-none-eabi
```

Host tests compare complete query and call traces for all four flag
combinations and exercise changes by both callbacks, including the 500 ms
flag changing in both directions. The target check compiles the `no_std`
library containing this module.

This is dispatch-order evidence only, not an implementation of application
effects or the SysTick tick model. It adds no ISR, MMIO, startup wiring, RF
operations, or hardware validation. The parent
[#52](https://github.com/m0hdy/uv-k5-rust-firmware/issues/52) remains open.
