# ST7565 startup execution

`src/st7565_startup.rs` provides the pinned startup plan, and
`src/st7565_startup_executor.rs` provides its allocation-free executor.
`execute_startup` runs the plan through typed reset, delay and SPI
initialization hooks plus the existing bounded line transport. It is not
connected to firmware startup and does not add MMIO or target startup wiring.
Its source is
[`ST7565_Init`, `cmds`, `ST7565_HardwareReset` and `ST7565_WriteByte`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/st7565.c),
and [`SPI_ToggleMasterMode` and the FIFO drain](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/spi.c)
at revision `7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`.

The SPI initialization effect is a typed caller hook. Reset levels are GPIO
RES levels; delays are requested milliseconds. The master-select effects
correspond to clearing and setting `MSR_SSN`, not disabling and enabling SPI.
Each command effect sets A0 low and uses the caller-supplied FIFO poll limit;
the drain effect uses the line transport's fixed `DRAIN_POLL_LIMIT`. If either
wait times out after `MSR_SSN` is cleared, execution restores the mode, returns
the corresponding `LineError`, and skips the remaining effects.

The last effect calls `fill_screen_handoff(0)` after restoring SPI mode. This
is the caller's handoff, not an executed clear. The pinned `DrawLine` uses its
`size_defVal` argument as both the loop bound and fill value, so this call
selects each page but writes **no** data bytes. The executor does not silently
write 128-byte zero lines; the caller owns any further action. The existing
`DisplayBlitPlan::draw_line` rejects empty payloads and is not used to model
the page-selection-only handoff.

The host traces check ordering, command values, repeated power writes and delay
requests, polling boundaries, and cleanup. They cannot establish elapsed time,
GPIO or SPI electrical behavior, successful FIFO completion, or panel
initialization; those require target and hardware evidence.

Focused checks from the repository root:

```sh
cargo test --locked --test st7565_startup --test st7565_startup_executor \
  --test st7565_display --test st7565_line_transport --test st7565_mmio_transport
cargo clippy --locked --test st7565_startup --test st7565_startup_executor \
  --test st7565_line_transport --test st7565_mmio_transport -- -D warnings
cargo clippy --locked --target thumbv6m-none-eabi --release -- -D warnings
```
