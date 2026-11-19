# Typed EEPROM read transport

[`src/eeprom_read_transport.rs`](../src/eeprom_read_transport.rs) executes the
existing [`ReadPlan`](../src/eeprom_transaction.rs) against a mockable
`I2cTransport`, borrowing the caller's output slice. It is standalone `no_std`,
allocation-free, and unexported from the firmware library. It reuses and
re-exports `ReadPlan` and `plan_read` from the existing planner; it does not
reconstruct device or address bytes.

The successful operation order follows
[`EEPROM_ReadBuffer`, driver/eeprom.c:24–39](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/eeprom.c#L24-L39)
at pinned public revision
[`egzumer/uv-k5-firmware-custom@7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3):

```text
START → write 0xa0 → write address high → write address low
      → repeated START → write 0xa1 → read buffer → STOP
```

Both START commands come from the plan; the executor distinguishes the second
as `repeated_start`. Each byte write returns a typed `Acknowledgement::Ack` or
`Acknowledgement::Nack`, separately from a transport error. Buffer reads fill the
borrowed slice directly. For a nonempty buffer, the transport must ACK every
received byte except the final byte, which it NACKs, matching the pinned
[`I2C_ReadBuffer` helper](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/i2c.c).
No retry, polling loop, allocation, or intermediate payload buffer is introduced
by the executor.

## Rust failure and output contract

The pinned EEPROM routine ignores write results. The following behavior is a
Rust contract, not a claim of C failure-path parity:

| Condition | Result and further operations |
| --- | --- |
| Output length differs from the planned read length | `DestinationLength { expected, actual }` before any I/O; output unchanged. Both shorter and longer slices are rejected. |
| Initial START fails | `Transport(error)`; no STOP or other operations. The transport must leave no active transaction on this failure. |
| Any byte write returns NACK | `NotAcknowledged`; attempt exactly one cleanup STOP and skip all remaining planned operations. |
| Byte write, repeated START, or buffer read fails | `Transport(error)`; attempt exactly one cleanup STOP and skip all remaining planned operations. |
| Cleanup STOP also fails | Discard the cleanup error and preserve the original error. No further operations. |
| Final planned STOP fails | Return its `Transport(error)` without retrying STOP. |

Transport methods must not panic or retry transfers. Cleanup is guaranteed for
returned intermediate errors after a successful initial START, not for panics.
A failed repeated START does not clear the active-transaction state. A failed
STOP may leave the physical bus state unknown; recovery policy belongs to the
caller or a future adapter.

Before the buffer read, failures leave output unchanged. A failed buffer read
may modify any part of output; no prefix length, rollback, clearing, or valid
payload is promised. A successful read must fill the entire slice. If the final
STOP then fails, output contains the complete response, but the operation still
returns an error. Callers must check the result before using output as a
successful transaction. Erased bytes (`0xff`) are valid data, not a transport
error or a missing-device sentinel.

## Length and address boundaries

The executor preserves the planner's full `u16` address range and `u8` length
range (`0..=255`), with no additional EEPROM capacity or span guard.

A zero-length plan still executes all eight planned operations, including
`read_buffer(&mut [])` and STOP. It requires an empty destination; the transport
must read no bytes for that empty slice. This preserves the planner and pinned
EEPROM routine's operation-level trace, **not** the pinned C helper's unsafe
zero-size behavior: `I2C_ReadBuffer` unconditionally performs a final byte read
and store even when `Size` is zero. That store is not reproduced in Rust.

## Scope and validation

This is the software-only read-execution boundary following the planner and
settings boot composition. The latter remains unchanged and is not wired to
this fallible executor. Tests exercise its startup addresses `0x0e70` and
`0x0e78` as separate eight-byte requests with distinct payloads. There are no
EEPROM data writes, bit-banged bus implementation, MMIO, startup changes,
hardware access, or release artifacts. Parent
[#43](https://github.com/m0hdy/uv-k5-rust-firmware/issues/43) remains open.

From the repository root:

```sh
cargo test --locked --test eeprom_read_transport
cargo clippy --locked --test eeprom_read_transport -- -D warnings
rustfmt --check --edition 2024 \
  src/eeprom_read_transport.rs tests/eeprom_read_transport.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings \
  src/eeprom_read_transport.rs \
  -o /tmp/eeprom_read_transport.rlib
```

Host tests assert literal ordered startup traces and payload routing,
preflight rejection without I/O, transport failure at every operation,
NACK at every byte write, successful and failed cleanup, partial-read output,
complete output after a failed final STOP, erased responses, zero-length
execution, and a maximum-length borrowed subslice with untouched edge bytes.
The isolated warnings-denied Cortex-M0 compile checks `no_std` compatibility;
it does not validate bus timing or hardware.
