# Offline periodic scheduler flags

## Pinned provenance

`src/periodic_scheduler_flags.rs` models only the counter and pending periodic
flags in [`scheduler.c:45–64`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/scheduler.c#L45-L64)
from public `egzumer/uv-k5-firmware-custom` at commit
`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`. Each tick wraps the `u32` counter
before setting the 10 ms flag, the 500 ms flag on multiples of 50, and the
40 ms flag on multiples of 4. Pending flags remain set until individually
acknowledged, so repeated events coalesce.

The model excludes all countdown and other side effects in the scheduler. It
does not claim elapsed real time, configure or handle an interrupt, access
MMIO, or connect to firmware application behavior.

## Focused validation

From the repository root, run the host tests and standalone Cortex-M0 compile:

```sh
cargo test --locked --test periodic_scheduler_flags
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/periodic_scheduler_flags.rs \
  -o /tmp/periodic_scheduler_flags.rlib
```

The host tests cover the first tick, each periodic boundary, a shared boundary,
coalescing, independent acknowledgement, wrap from `u32::MAX` to zero, and the
ordinary tick after wrap. The standalone compile checks the isolated `no_std`
module with warnings denied.
