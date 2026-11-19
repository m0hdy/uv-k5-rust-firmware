# ST7565 offline startup sequence

`src/st7565_startup.rs` is an allocation-free, standalone plan. It does not
perform I/O or connect to firmware startup. Its source is
[`ST7565_Init`, `cmds`, `ST7565_HardwareReset` and `ST7565_WriteByte`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/st7565.c),
and [`SPI_ToggleMasterMode` and the FIFO drain](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/spi.c)
at revision `7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`.

The SPI initialization effect is a handoff to the existing SPI0 startup
adapter. Reset levels are GPIO RES levels; delays are requested milliseconds.
The master-select effects correspond to clearing and setting `MSR_SSN`, not
disabling and enabling SPI. Each command effect represents one source
`ST7565_WriteByte`: set A0 low, wait for transmit FIFO space, then write the
byte. The drain effect represents the source's bounded FIFO status wait; the
plan does not model its polling outcome or implement a timeout policy.

The last effect is the source call `ST7565_FillScreen(0)`, not an executed
clear. The pinned `DrawLine` uses its `size_defVal` argument as both the loop
bound and fill value, so this call selects each page but writes **no** data
bytes. A consumer choosing to actually clear the screen can instead supply a
128-byte zero line to the existing `DisplayBlitPlan::draw_line` for each of
pages 0 through 7; that is an intentional behavioral difference, not a trace
of the pinned fill implementation. The plan does not execute this handoff.

The host trace checks ordering, command values, repeated power writes and delay
requests. It cannot establish elapsed time, GPIO or SPI electrical behavior,
successful FIFO completion, or panel initialization; those require target and
hardware evidence.

Focused checks from the repository root:

```sh
cargo test --locked --test st7565_startup --test st7565_display --test spi0_startup_adapter
cargo clippy --locked --test st7565_startup -- -D warnings
rustfmt --check --edition 2024 src/st7565_startup.rs tests/st7565_startup.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/st7565_startup.rs -o /tmp/st7565_startup.rlib
```
