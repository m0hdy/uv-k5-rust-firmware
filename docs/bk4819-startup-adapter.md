# BK4819 cold-initialization adapter

[`src/bk4819_startup_adapter.rs`](../src/bk4819_startup_adapter.rs) executes
the pinned `BK4819_Init` register sequence through the existing
[`bk4819_register_transport`](../src/bk4819_register_transport.rs). It is
standalone and is not exported from `src/lib.rs` or connected to firmware
startup. The caller supplies an accurate, non-panicking one-microsecond delay
and its mutable BK4819 GPIO-output shadow; no timing default, global state,
allocation, or board policy is provided.

The sequence follows `BK4819_Init`, `BK4819_InitAGC(false)`, and
`BK4819_SetAGC(true)` in
[`driver/bk4819.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/bk4819.c#L51-L294)
at pinned revision
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
It begins with three separate fresh GPIOC DATA read-modify-writes setting SCN,
SCL, then SDA high. It then performs the reset and remaining writes in source
order, including the active `#if 1` DTMF coefficients. InitAGC's FM settings
are fixed to the called `false` branch.

`SetAGC(true)` always reads register `0x7e` through the GPIO transport. If bit
15 is clear, it issues no `0x7e` write and initialization continues. If bit 15
is set, it writes `(fresh_value & 0x0fff) | 0x3000`, preserving the low twelve
bits. The caller's GPIO-output shadow is set to `0x9000` before register
`0x33` is written with that value.

## Target prerequisites and scope

The unsafe ARM bare-metal `initialize_mmio` entry uses volatile accesses to the
existing DP32G030 GPIOC/PORTCON mapping. Before calling it, ensure those
mappings are accessible, GPIOC's SYSCON clock gate is enabled, and GPIOC0/1/2
are muxed to GPIO and configured as outputs with input-enable and open-drain
disabled. The transport temporarily changes C2 input-enable and direction for
the `0x7e` read and restores them. The caller must own the complete BK4819 bus
and serialize the call against all GPIOC DATA/DIR and PORTC input-enable/
open-drain writers, including interrupts and unrelated-bit writers, because
the operations are full-word read-modify-writes. No GPIO serial algorithm is
duplicated here.

This is cold initialization only: it is not runtime AGC/AM policy, mode
control, RF/application integration, firmware startup wiring, or hardware
acceptance. Offline tests and target compilation do not execute device code.
Initialization, mode control, application/device follow-up, and bench
validation remain distinct; no completion of parent issue #45 is claimed.

## Focused validation

From the repository root, with the `thumbv6m-none-eabi` target installed:

```sh
cargo test --locked --test bk4819_startup_adapter
cargo clippy --locked --test bk4819_startup_adapter -- -D warnings
rustfmt --check --edition 2024 \
  src/bk4819_startup_adapter.rs tests/bk4819_startup_adapter.rs \
  tests/fixtures/bk4819_startup_adapter_target.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings tests/fixtures/bk4819_startup_adapter_target.rs \
  -o /tmp/bk4819_startup_adapter_target.rlib
```

The focused host test decodes transactions emitted by the actual GPIO
transport, scripts the returned `0x7e` bits, and compares the full register
effect sequence for both AGC branches. It also checks the initial GPIO
read-modify-writes and caller-owned shadow. The warning-denied Cortex-M0
fixture instantiates the unsafe MMIO path with an external delay symbol and is
compiled only.
