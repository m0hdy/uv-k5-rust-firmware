# BK1080 receive execution

`src/bk1080_receive_adapter.rs` executes BK1080 register operations over the
existing `I2cTransport`, using the tune plan in `src/bk1080_tuning.rs`. The
register protocol and tune behavior follow `BK1080_SetFrequency`,
`BK1080_Mute`, and `BK1080_GetFrequencyDeviation` in
[`driver/bk1080.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/bk1080.c)
and [`driver/bk1080-regs.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/bk1080-regs.h)
at pinned reference
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
The adapter is unexported and is not connected to startup or firmware entry.

## Register operations

`write_register` sends START, `0x80`, `(register << 1)`, the value's high
byte, the value's low byte, then STOP. `read_register` sends START, `0x80`,
`((register << 1) | 1)`, reads two bytes through `read_buffer`, then STOP.
There is no EEPROM register-address phase or repeated START. The returned
`u16` preserves the receiver's big-endian register value. The transport's
two-byte read acknowledges the first byte and does not acknowledge the final
byte.

Each register operation has its own transaction. A failed START is returned
without STOP. A failed byte write, NACK, or failed buffer read after START
causes exactly one best-effort cleanup STOP; a cleanup error does not replace
the original failure. A failed final STOP is returned without retry. No
operation following a failure runs, and failed reads never return a partial
value or a default.

`set_frequency` reads register `0x05` on the bus and passes that fresh value to
`TunePlan::new`. It then executes the plan's ordered configuration write,
channel write, caller-provided 10 ms delay, and tune-bit channel write.
Unrelated configuration bits, the model's band modulo/mask and `u16` wrapping
behavior, and the reference's lack of out-of-range rejection are preserved.
The adapter exposes `Register::from_index` for the full valid initialization
range 0–33; out-of-range indices return `None`. In particular,
`Register::Internal25` is register index decimal 25 (`0x19`), whose I2C
register-address byte is `0x32`. This enables later initialization/calibration
code to use the same typed register operations without duplicating bus framing.

The receive executor itself requires an initialized and powered receiver.
The separate standalone nonzero-frequency first-init executor is documented
in [`bk1080-receive-init-adapter.md`](bk1080-receive-init-adapter.md).
`set_muted` executes the existing mute command values.
`read_frequency_deviation` reads register `0x07` and passes it with the
supplied base frequency to the existing `frequency_deviation` function. These
operations do not change PB15 or power state and do not provide audio-path
integration.

## Target GPIO convenience

The ARM bare-metal-only `set_frequency_mmio`, `set_muted_mmio`, and
`read_frequency_deviation_mmio` functions construct the existing
`GpioI2cIo::new_target` GPIOA10/11 owner, connect it through
`I2cBitbangTransport`, and call `handoff_to_keypad` on success or error. They
require explicit one-microsecond and, for tuning, millisecond delay providers;
there is no implicit clock, pin setup, or delay implementation.

Before calling, the caller must ensure DP32G030 GPIOA and PORTCON mappings are
accessible and the GPIOA clock is enabled. PA10 must be muxed as GPIOA10, and
PA11 as GPIOA11; both must be initialized as GPIO outputs, with input-enable
and open-drain disabled on both pins. These are the board I2C pin states
documented in [`i2c-bitbang-flow.md`](i2c-bitbang-flow.md). The receiver must
already be initialized and powered.

The unsafe target caller must exclusively own the I2C bus and the keypad-shared
PA10/PA11 pins. Serialize the entire operation—including every bus transfer,
error cleanup, target delays, keypad handoff, and owner drop—against interrupts
and every other writer to GPIOA DATA/DIR and PORTA input-enable/open-drain.
These operations use full-register read-modify-writes, so unrelated-bit
writers must be serialized too. The supplied one-microsecond delay must be
accurate, infallible and non-panicking, and must not access those shared
registers. The millisecond provider must be accurate, infallible and
non-panicking for the requested 10 ms wait, must not access the shared GPIO
registers, and the same exclusive ownership must remain in force throughout
that delay. Handoff restores the board's keypad pin mode; it does not resume
keypad scanning or arbitrate ownership.

## Focused validation

From the repository root, with the `thumbv6m-none-eabi` target installed:

```sh
cargo test --locked --test bk1080_receive_adapter
cargo clippy --locked --test bk1080_receive_adapter -- -D warnings
cargo fmt --all -- --check

ROOT="$(pwd)"
RUSTFLAGS='-D warnings' cargo build --locked --lib --target thumbv6m-none-eabi
HARNESS="$(mktemp -d /tmp/bk1080-receive-target-XXXXXX)"
cat > "$HARNESS/lib.rs" <<EOF
#![no_std]
pub use uv_k5_rust_firmware::{eeprom_i2c_bridge, eeprom_read_transport, i2c_bitbang_flow};
#[path = "$ROOT/src/bk1080_receive_adapter.rs"]
pub mod bk1080_receive_adapter;
EOF
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi -D warnings \
  --extern "uv_k5_rust_firmware=$ROOT/target/thumbv6m-none-eabi/debug/libuv_k5_rust_firmware.rlib" \
  "$HARNESS/lib.rs" -o "$HARNESS/libbk1080_receive_adapter.rlib"
```

The standalone harness imports the unexported adapter and compiles its
target-only functions against the real library modules. Host tests check exact
register frames, the fresh configuration read and tune/delay order, mute and
deviation execution, failure cleanup and short-circuiting, and a register read
through the bitbang bridge including the payload ACK/NACK. This evidence does
not establish physical timing, electrical behavior, or hardware operation.
No flashing, bootloader work, device execution, or installable release is part
of this adapter.
