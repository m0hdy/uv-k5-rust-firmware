# SARADC startup adapter

`src/adc_startup_adapter.rs` implements the register and IRQ operations performed
by `BOARD_ADC_Init()` in `board.c`, together with `ADC_Configure()`,
`ADC_Enable()`, and `ADC_SoftReset()` in `driver/adc.c`. It follows
[`board.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/board.c),
[`driver/adc.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/adc.c),
and the DP32G030 [`saradc.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/saradc.h)
and [`syscon.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/syscon.h)
at pinned revision
[`7607f0a`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).

## Execution order

1. Read-modify-write the SARADC clock gate.
2. Read-modify-write CFG to disable ADC.
3. Read `SYSCON_CLK_SEL`, apply the driver's PLL/sample-clock remapping, and
   write the selector with the board's DIV2 selector.
4. Read-modify-write CFG with channels 4 and 9, eight-sample averaging,
   single conversion, channel memory, internal sample clock, 15-cycle window,
   one-cycle setup, CPU trigger, and DMA disabled.
5. Write the caller-provided `EXTTRIG_SEL` directly, zero-extended to 32 bits.
6. Read-modify-write offset-valid and KD-valid calibration fields to valid.
7. Write all ones directly to IF (write-one-to-clear), without reading IF.
8. Read-modify-write IE with the board's disabled interrupt fields, read IE
   back, then disable NVIC's SARADC IRQ only if that readback is zero;
   otherwise enable it.
9. Read-modify-write CFG to enable ADC.
10. Read-modify-write START to assert soft reset, then read it again and
    read-modify-write it to deassert reset.

Every read-modify-write uses a fresh read, including each CFG update and both
reset operations. `apply_with_io` remains the single sequence executor. The
bare-metal ARM entry, `unsafe fn apply_to_hardware(exttrig_sel: u16)`, runs it
through aligned volatile 32-bit accesses as declared by the pinned BSP.

The pinned [`irq.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/irq.h)
assigns SARADC IRQ 4. The pinned CMSIS [`core_cm0.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/external/CMSIS_5/CMSIS/Core/Include/core_cm0.h)
enables it with a write of `0x10` to ISER (`0xe000_e100`) and disables it with a
write of `0x10` to ICER (`0xe000_e180`) followed by `DSB` and `ISB`. The
single-bit writes leave other interrupt enables unchanged; they do not touch
NVIC pending state or priority.

The pinned `BOARD_ADC_Init()` does not assign `Config.EXTTRIG_SEL`, although
`ADC_Configure()` writes that member directly to the hardware register. Its
value therefore cannot be established from the board initializer. The adapter
requires each caller to supply `exttrig_sel`; it does not invent a default or
claim exact parity for that field. Choosing a production value remains an
unresolved policy decision.

## Target call prerequisites

- Run on the DP32G030/Cortex-M0 mapping in a privilege and clock state that
  permits SYSCON and NVIC accesses at entry and SARADC access as soon as the
  device gate is enabled. The selected sample clock's source must be running;
  the adapter opens the gate and selects the divider, not the upstream clock
  source.
- Serialize the whole call against every writer to the touched SYSCON and
  SARADC registers, including interrupt handlers and code updating unrelated
  fields in the shared clock registers. Also serialize other changes to the
  SARADC NVIC line. Exclusive ownership of the ADC pins alone does not protect
  the read-modify-write operations.
- If the interrupt can be delivered, install a valid handler in the active
  vector for external IRQ 4 (vector table index 20). Preserved IE bits can
  cause NVIC enable before the final CFG and reset writes, and the NVIC line
  might already be enabled or pending on entry. Keep delivery masked if needed
  until the handler and peripheral are ready, and prevent it from racing
  startup. The adapter does not install vectors or clear pending state.

The adapter remains an internal module and is called by the board startup
dispatcher at `Call::AdcInit`, after backlight and before the concrete ST7565
startup stage.
`BootConfig::exttrig_sel` is forwarded unchanged to this executor; the pinned
board initializer still supplies no production value. The adapter does not
configure ADC channel pins, start battery sampling, or poll for conversion
completion. Host tests and link-only inspection do not establish hardware
timing or bench behavior.

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

The host tests check scripted fresh reads at the logical boundary and full
ordered word-access traces through the physical mapper. The physical mock
exercises trigger forwarding, reserved and unrelated bits, direct W1C writes
to IF, and both IRQ branches while preserving neighboring NVIC enables.
