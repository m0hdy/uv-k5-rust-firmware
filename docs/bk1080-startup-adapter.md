# BK1080 startup shutdown adapter

`src/bk1080_startup_adapter.rs` implements the zero-frequency branch of
`BK1080_Init(0, 0)`, called by `BK1080_Init0`, from
[`driver/bk1080.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/bk1080.c).
The GPIO operation follows
[`driver/gpio.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/gpio.h)
and the DP32G030
[`bsp/dp32g030/gpio.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/gpio.h)
at pinned revision
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).

## Operation and errors

`shutdown_with_io` uses the existing `I2cTransport` and a word-I/O boundary.
On success it sends START, `80 04 02 41`, STOP: a write of `0x0241` to
BK1080 register `0x02`. It then freshly reads the 32-bit GPIOB DATA register
at `0x40060800` and writes that value with bit 15 set. There is no EEPROM word
address, repeated START, data read, register table, tuning operation or
millisecond delay.

The pinned C ignores acknowledgements; its behavior does not define transport
error handling. The Rust policy is explicit: a failed START is not stopped;
after a successful START, a NACK or write error triggers one best-effort
cleanup STOP, whose failure does not replace the original error. A failed
final STOP is returned and is not retried. No later bus operations are
attempted. In all cases PB15 is asserted exactly once after transaction cleanup,
using a fresh GPIOB DATA read, before the error is returned. Transport and
word-I/O implementations must not panic. A failed STOP does not guarantee
that a fallible transport physically released the bus.

## Target prerequisites and scope

The ARM bare-metal-only unsafe `shutdown_mmio` uses
`GpioI2cIo::new_target` and `I2cBitbangTransport`, then hands the shared
GPIOA10/11 I2C pins back to the keypad on success or error. Its target
bitbang transport has infallible pin operations but can report a NACK.
The caller supplies an accurate, non-panicking one-microsecond delay that does
not access shared registers; no clock, pin, or millisecond-delay policy is
provided here.

Before calling, the caller must ensure that the DP32G030 GPIOA, GPIOB and
PORTCON registers are mapped and accessible with their clocks enabled;
PA10/PA11 must be muxed and initialized for the existing GPIO I2C owner,
and PB15 must be initialized as a GPIO output. The caller must exclusively
own the shared I2C bus and suspend keypad use. Serialize the entire operation,
cleanup and keypad handoff against all users and interrupt handlers that write
GPIOA DATA/DIR, PORTA input-enable/open-drain, or GPIOB DATA, including
unrelated-bit writers: these are whole-register read-modify-writes. The
adapter does not arbitrate ownership or resume keypad scanning.

This unexported callable backend is not board or firmware entry wiring, full
FM initialization, receiver/audio parity, or hardware validation.

## Focused validation

From the repository root, with the installed `thumbv6m-none-eabi` target:

```sh
ROOT="$(pwd)"
cargo test --locked --test bk1080_startup_adapter
cargo clippy --locked --test bk1080_startup_adapter -- -D warnings
cargo test --locked --test i2c_bitbang_flow --test eeprom_i2c_bridge \
  --test eeprom_read_transport
rustfmt --check --edition 2024 "$ROOT/src/bk1080_startup_adapter.rs" \
  "$ROOT/tests/bk1080_startup_adapter.rs"

RUSTFLAGS='-D warnings' cargo build --locked --lib --target thumbv6m-none-eabi
HARNESS="$(mktemp -d /tmp/bk1080-target-XXXXXX)"
printf '#![no_std]\npub use uv_k5_rust_firmware::{eeprom_i2c_bridge, eeprom_read_transport, i2c_bitbang_flow};\n#[path = "%s/src/bk1080_startup_adapter.rs"]\npub mod bk1080_startup_adapter;\n' "$ROOT" > "$HARNESS/lib.rs"
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi -D warnings \
  --extern "uv_k5_rust_firmware=$ROOT/target/thumbv6m-none-eabi/debug/libuv_k5_rust_firmware.rlib" \
  "$HARNESS/lib.rs" -o "$HARNESS/libbk1080.rlib"
```

The harness imports the actual unexported module and its existing library
dependencies, so it compiles the target-guarded convenience as well as the
executor. On 2026-11-19 these commands passed with rustc 1.99.1
(`58cf8f9c3 2026-10-13`). The host tests verify the complete bus/GPIO trace,
each failure position and cleanup precedence, and ACK/NACK through the existing
bitbang bridge. They do not execute code on hardware.
