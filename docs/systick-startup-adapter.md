# SysTick startup adapter

[`src/systick_startup_adapter.rs`](../src/systick_startup_adapter.rs) models
the `SYSTICK_Init()` default from
[`driver/systick.c` at pinned revision
`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/systick.c#L24-L28).
The startup sequence uses the existing 24-bit reload validation and follows
CMSIS Cortex-M0 `SysTick_Config`: LOAD, SysTick priority, VAL, then CTRL.
For the default 480,000 ticks, the trace writes LOAD `479999`, updates the
SysTick priority field to the lowest priority, clears VAL, and enables the
core-clock timer and its interrupt (`CTRL = 7`). The operation order and
priority update follow [CMSIS Cortex-M0 `SysTick_Config` and
`NVIC_SetPriority`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/external/CMSIS_5/CMSIS/Core/Include/core_cm0.h).
The delay multiplier `48` remains distinct from the period and does not
establish or measure the core frequency. This module contains no delay loop.

CMSIS Cortex-M0 accesses system-handler priorities as 32-bit words. The
adapter therefore updates only SHPR3's SysTick byte (`mask = 0xff000000`,
priority value `0xc0000000` for two implemented priority bits) with a word
read/modify/write, preserving the neighboring exception priorities. Invalid
reload periods produce an error before any operation is emitted.

The volatile adapter is limited to ARM bare-metal targets and is unsafe to call.
Its preconditions are a DP32G030/Cortex-M0 core-register mapping, exclusive
early-startup access to the core registers, and a valid SysTick handler and
vector before enabling SysTick interrupts. The model is not connected to a
boot call site or ISR. No firmware artifact, MMIO execution, RF, or bench
validation is included.

Run the host tests and compile the standalone module for Cortex-M0 with warnings
denied:

```sh
cargo test --locked --test systick_startup_adapter
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings --emit=llvm-ir src/systick_startup_adapter.rs \
  -o /tmp/systick_startup_adapter.ll
grep -n 'load volatile i32\\|store volatile i32' \
  /tmp/systick_startup_adapter.ll
```

The tests cover the literal default trace and its order, minimum and maximum
valid reloads, both invalid endpoints with no writes, and preservation of
neighboring exception-priority fields.
