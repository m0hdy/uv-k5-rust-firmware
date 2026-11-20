# CRC startup adapter

`src/crc_startup_adapter.rs` implements `CRC_Init` from
[`driver/crc.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/crc.c)
using the DP32G030 definitions in
[`bsp/dp32g030/crc.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/crc.h),
at pinned revision
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
The referenced files are copyright 2023 Dual Tachyon and licensed under
Apache-2.0; this adapter independently implements their initialization
behavior.

## Ordered writes

1. Assign `0x00000100` to `CRC_CR` at `0x40003000`: engine disabled,
   normal input and output transforms, 8-bit data width, CRC-16-CCITT.
2. Assign zero to `CRC_IV` at `0x40003004`.

Both operations are complete 32-bit assignments, not read-modify-writes.
The executor does not read registers, access `CRC_DATAIN` or `CRC_DATAOUT`,
calculate a CRC, or configure clocks. The ARM bare-metal-only unsafe
`apply_to_hardware` entry uses the same executor with volatile word writes.

## Target prerequisites and scope

The caller must ensure the DP32G030 registers are mapped and writable as
aligned 32-bit words in the current privilege state and that the CRC clock is
already enabled. The caller must own the whole CR and IV registers and
serialize the entire call against all other CRC users, including interrupt
handlers. Owning individual fields is insufficient because both writes
replace whole registers.

This module is unexported; the integrated board startup dispatcher calls it
when `ENABLE_UART` or `ENABLED_AIRCOPY` is present. The earlier device-clock
assignment already enables CRC bit 27. The default firmware entry does not
invoke the dispatcher. Offline checks do not establish physical CRC behavior.

## Focused validation

From the repository root:

```sh
cargo test --locked --test crc_startup_adapter
cargo clippy --locked --test crc_startup_adapter -- -D warnings
rustfmt --check --edition 2024 \
  src/crc_startup_adapter.rs tests/crc_startup_adapter.rs
rustc --edition=2024 --crate-type lib --target thumbv6m-none-eabi \
  -D warnings src/crc_startup_adapter.rs --out-dir target/crc-startup-check
```

The host test starts with nonzero register values and checks the complete
ordered word-write trace and unchanged data registers. The write-only
boundary permits no reads. The standalone target check compiles the actual
guarded volatile entry despite the module being unexported.

On 2026-11-19, the target check above passed with rustc 1.99.1
(`58cf8f9c3 2026-10-13`). An optimized standalone assembly inspection also
showed two ordered word stores to `0x40003000` and `0x40003004`, with no
CRC register loads; the artifact was not executed.
