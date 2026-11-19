# Offline SARADC startup register sequence

`src/adc_startup_adapter.rs` models the register and IRQ operations performed
by `BOARD_ADC_Init()` in `board.c`, together with `ADC_Configure()`,
`ADC_Enable()`, and `ADC_SoftReset()` in `driver/adc.c`. It follows
[`board.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/board.c),
[`driver/adc.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/adc.c),
and the DP32G030 [`saradc.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/saradc.h)
and [`syscon.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/syscon.h)
at pinned revision
[`7607f0a`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).

## Modeled order

1. Read-modify-write the SARADC clock gate.
2. Read-modify-write CFG to disable ADC.
3. Read `SYSCON_CLK_SEL`, apply the driver's PLL/sample-clock remapping, and
   write the selector with the board's DIV2 selector.
4. Read-modify-write CFG with channels 4 and 9, eight-sample averaging,
   single conversion, channel memory, internal sample clock, 15-cycle window,
   one-cycle setup, CPU trigger, and DMA disabled.
5. Write the caller-provided `EXTTRIG_SEL`.
6. Read-modify-write offset-valid and KD-valid calibration fields to valid.
7. Write all ones to IF.
8. Read-modify-write IE with the board's disabled interrupt fields, read IE
   back, then disable NVIC's SARADC IRQ only if that readback is zero;
   otherwise enable it.
9. Read-modify-write CFG to enable ADC.
10. Read-modify-write START to assert soft reset, then read it again and
    read-modify-write it to deassert reset.

Every read-modify-write uses a fresh read, including the two CFG configuration
updates and both reset operations. `RegisterIo` and `Interrupt` are typed,
address-free interfaces; the module has no volatile or other MMIO backend.
Host tests use a mock that records each read, write, and IRQ action.

The pinned `BOARD_ADC_Init()` does not assign `Config.EXTTRIG_SEL`, although
`ADC_Configure()` writes that member directly to the hardware register. Its
value therefore cannot be established from the board initializer. The model
requires each caller to supply `exttrig_sel`; it does not invent a default or
claim exact parity for that field. Choosing a production value remains an
unresolved policy decision.

This is an isolated software model, not boot wiring or hardware validation. It
does not poll for conversion completion, access target registers, or depend on
another board witness.

## Focused validation

From the repository root:

```sh
cargo test --locked --test adc_startup_adapter
cargo clippy --locked --test adc_startup_adapter -- -D warnings
rustfmt --check --edition 2024 \
  src/adc_startup_adapter.rs tests/adc_startup_adapter.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/adc_startup_adapter.rs -o /tmp/adc_startup_adapter.rlib
```
