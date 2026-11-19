# ST7565 single-line transport

[`src/st7565_line_transport.rs`](../src/st7565_line_transport.rs) executes
only the checked `DisplayBlitPlan::draw_line` path. It is allocation-free,
`no_std`, and independent of a hardware implementation. Its ordering follows
the pinned [`ST7565_DrawLine` and `DrawLine`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/st7565.c#L31-L47)
and [`ST7565_SelectColumnAndLine`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/st7565.c#L197-L207).
The drain bound follows [`SPI_WaitForUndocumentedTxFifoStatusBit`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/spi.c#L46-L58);
the register definitions are in [`spi.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/spi.h).

`draw_line` takes page, visible-screen column, payload, and a nonzero
per-byte FIFO poll limit. The existing plan validates page 0–7, column 0–127,
a nonempty payload, and that the payload fits on the line. Invalid geometry
returns `InvalidGeometry` without touching the transport. In particular, an
empty payload is not a valid zero-length line. The plan supplies the three
command bytes, including the controller's four-column offset, and the data.

The `LineTransport` implementation must provide infallible, non-panicking
operations and exclusive access for the duration of a call:

* `set_spi_mode(false)` and `set_spi_mode(true)` correspond to the pinned
  driver's `SPI_ToggleMasterMode` calls, specifically the `MSR_SSN` setting;
  they do not enable or disable the SPI peripheral.
* `set_a0(Command)` selects low A0; `set_a0(Data)` selects high A0.
* Each `fifo_ready` call reads transmit FIFO status once and returns true when
  the TFF bit is not full; `write_byte` writes one byte.
* Each `drain_busy` call reads the undocumented IF bit `0x20` once and returns
  true while it is set.

After switching mode, the executor selects command A0, polls FIFO before each
command byte, drains, selects data A0, polls FIFO before each payload byte,
drains again, and restores mode. On either timeout it stops immediately,
restores mode, and returns `FifoTimeout` or `DrainTimeout`. A timeout may leave
a partial transfer and pending bytes; restoring the mode does not imply a
completed or rolled-back display update. The caller must handle recovery.
Mode restoration cannot be guaranteed if a transport operation panics or does
not return.

The FIFO poll limit is a caller-selected Rust safety policy: upstream waits
without a bound. It counts status reads separately for each byte, including
the successful read. Each drain permits exactly 100,001 reads, including a
successful final read, matching the pinned loop's maximum. Unlike the pinned
routine, which silently continues after exhausting that loop, this executor
reports a drain timeout and performs no further writes.

This module does not implement MMIO, boot wiring, other blit entry points, or
hardware validation. Run focused host tests and a warnings-denied Cortex-M0
compile from the repository root:

```sh
cargo test --locked --test st7565_line_transport --test st7565_display
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/st7565_line_transport.rs -o /tmp/st7565_line_transport.rlib
```
