# ST7565 MMIO line transport

[`src/st7565_mmio_transport.rs`](../src/st7565_mmio_transport.rs) adapts the
bounded [`LineTransport`](../src/st7565_line_transport.rs) executor to only
the DP32G030 SPI0 and GPIOB registers used by the pinned ST7565 line path. It
is used by the integrated board [display startup](st7565-startup.md) and the
separate callable [welcome execution](startup-version-display.md#callable-welcome-execution)
boundary.

The register addresses and fields follow
[`spi.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/spi.c#L46-L109),
[`st7565.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/st7565.c#L31-L47),
[`spi.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/spi.h),
and [`gpio.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/gpio.h)
at pinned revision
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
SPI mode changes update only `SPI_CR_MSR_SSN` (bit 12), not `MSTR`; A0 is
GPIOB DATA bit 9. Each mode/A0 update preserves the other register bits. FIFO
status and the undocumented drain-busy flag are read from `FIFOST` bit 4 and
`IF` mask `0x20`. The write-data register and status registers are accessed
with 32-bit operations, and each payload byte is zero-extended into the
32-bit `WDR` register.

The generic `RegisterIo` boundary allows host tests to record all reads and
writes. The unsafe volatile entry exists only for `arm`/`none` targets and does
not configure GPIO direction or pin mux, initialize SPI, enable clocks,
arbitrate the bus, or establish exclusive ownership. Before calling it, the
caller must ensure:

* The target is a DP32G030 with the register map listed by
  `RegisterAddress::address`.
* GPIOB bit 9 is configured as an output and muxed as the ST7565 A0 GPIO, and
  the SPI0 pins are muxed for the display.
* GPIOB and SPI0 peripheral clocks are enabled.
* SPI0 has already been configured for the display, with FIFO/status flags
  active and no pending transfer; the SPI bus is idle.
* This call has exclusive ownership of SPI0 CR, WDR, IF, FIFOST and GPIOB DATA
  until the bounded line transfer returns.

The entry applies no display startup, publishes no hardware witness, and makes
no claim of device-level validation. A transfer timeout may leave a partial
line in the display/FIFO; the executor restores `MSR_SSN` before returning.

Run focused host tests and format/lint checks from the repository root:

```sh
cargo test --locked --test st7565_mmio_transport --test st7565_line_transport
cargo clippy --locked --test st7565_mmio_transport -- -D warnings
cargo fmt --all -- --check
```

Compile the adapter for Cortex-M0 and retain the target-only entry and volatile
operations for inspection:

```sh
cargo rustc --locked --lib --target thumbv6m-none-eabi -- \
  -D warnings -C opt-level=2 -C link-dead-code=yes --emit=asm
```

The assembly is emitted under `target/thumbv6m-none-eabi/debug/deps/` as
`uv_k5_rust_firmware-*.s`. Inspect it for 32-bit volatile loads/stores at the
listed register addresses and the bounded FIFO/drain polling paths. This
compiles and inspects the implementation only; it does not access a device or
produce a firmware image.
