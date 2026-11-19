# SysTick delay adapter

[`src/systick_delay_adapter.rs`](../src/systick_delay_adapter.rs) provides
counter-polling implementations of the existing I2C `Delay1us` and ST7565
`DelayMs` traits. It is intentionally unexported and is not connected to
firmware startup. The read-only reference is
[`driver/systick.c` at revision
`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/systick.c).

Each delay snapshots LOAD and then VAL, polls VAL until it changes, and
accumulates samples with the existing `systick_timing::elapsed_after_sample`
arithmetic. That helper retains the pinned C routine's one-tick-short wrap
calculation and `u32` elapsed accumulation. A zero or overflowed target still
waits for one changed VAL sample, as the C do/while does. The millisecond
adapter performs wrapping `u32` multiplication by 1,000 before converting
microseconds to ticks.

Construction requires an explicit `NonZeroU32` ticks-per-microsecond value.
The caller must supply a verified integer tick rate for its configured clock;
neither the reference's 480,000 reload nor its multiplier of 48 establishes
or verifies core frequency or wall-clock timing, and the rate must remain
stable throughout each delay. Passing 48 reproduces the pinned tick conversion,
while other values change the actual polling target. These values alone make
no bench-timing claim.

The target-only `VolatileSysTickCounter` reads LOAD at `0xE000E014` and VAL at
`0xE000E018` with volatile 32-bit reads. Its unsafe constructor requires the
addresses to map to the target SysTick. The caller must also ensure SysTick is
enabled, free-running, and configured with a valid 24-bit LOAD; LOAD and
control configuration must remain stable, and no other code may write LOAD or
VAL or reconfigure the counter during delays. No control-register reads or
timer writes are performed. I/O implementations must be infallible and
non-panicking; if interrupts can run, their handlers and vectors must be
valid. A stopped counter can wait indefinitely, as in the C implementation.
No device execution or timing validation is implied.

Run the focused tests and checks from the repository root:

```sh
cargo test --locked --test systick_delay_adapter
cargo test --locked --test systick_timing --test systick_startup_adapter
cargo clippy --locked --test systick_delay_adapter -- -D warnings
cargo fmt --all -- --check
```

Compile the actual module, including its target-only volatile implementation
and both existing trait implementations, in a `no_std` harness for Cortex-M0:

```sh
cargo build --locked --lib --target thumbv6m-none-eabi
cat > /tmp/systick_delay_adapter_harness.rs <<EOF
#![no_std]
extern crate uv_k5_rust_firmware;
#[path = "$PWD/src/systick_delay_adapter.rs"]
pub mod systick_delay_adapter;
EOF
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings \
  --extern uv_k5_rust_firmware=target/thumbv6m-none-eabi/debug/libuv_k5_rust_firmware.rlib \
  /tmp/systick_delay_adapter_harness.rs \
  -o /tmp/libsystick_delay_adapter_harness.rlib
```

The tests inject counter traces for repeated samples, decrement and wrap,
non-48 calibration, millisecond conversion and overflow, and the zero-target
changed-sample rule. They exercise both existing delay traits; they do not
establish configured device timing.
