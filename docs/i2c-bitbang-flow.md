# Offline I2C bitbang flow

[`src/i2c_bitbang_flow.rs`](../src/i2c_bitbang_flow.rs) models
[`driver/i2c.c:23–147`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/i2c.c#L23-L147)
at pinned public revision
[`egzumer/uv-k5-firmware-custom@7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
It is an allocation-free `no_std` module exported by the firmware library.
The `I2cIo` trait supplies infallible typed operations for SCL, SDA, PA11 input
enable, open drain and direction, SDA samples, and one-microsecond delays.
[`target_gpio`](../src/i2c_bitbang_flow/target_gpio.rs) implements that
interface for the target GPIOA and PortCon registers.

START sets SDA high, sets SCL high, clears SDA, then clears SCL, with a delay
after each operation. STOP clears SDA, clears SCL, sets SCL, then sets SDA,
again delaying after each operation. A write sends eight bits most significant
first, then switches PA11 to input and samples SDA up to 255 times without
intervening delays. Low is ACK; 255 high samples return NACK. Both paths lower
SCL, delay, restore PA11 input-enable/open-drain/direction in that order, and
set SDA high.

A read switches PA11 to input, reads eight most-significant-first bits, then
restores PA11 and clocks out a low ACK for a non-final byte or a high NACK for
the final byte. `read_buffer` delays before every byte and ACKs all but the
last. Unlike the C helper's unconditional final read and store for size zero,
an empty Rust slice performs no operations and stores nothing.

## Target GPIO backend

The pinned
[`driver/i2c.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/i2c.c)
uses PA10 for SCL and PA11 for SDA. The pinned GPIO and PortCon headers map
GPIOA DATA/DIR to `0x40060000`/`0x40060004`, PORTA input-enable to
`0x400B0100`, and PORTA open-drain to `0x400B0400`. SCL uses bit 10 (`0x400`);
SDA uses bit 11 (`0x800`). Each operation reads the current 32-bit register
value before changing only its pin mask. SDA samples come from GPIOA DATA bit 11.
Volatile register reads and writes are compiled only for `arm`/`none` targets.

Before `GpioI2cIo::new_target`, the caller must have completed the target board
setup: GPIOA clock enabled, PA10/PA11 muxed to GPIOA10/GPIOA11, both pins
outputs, and PA11 input-enable/open-drain disabled. These are the relevant
states established by pinned
[`BOARD_PORTCON_Init` and `BOARD_GPIO_Init`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/board.c)
before normal I2C use.

PA10/PA11 are shared with the keypad. Creating the target backend is unsafe:
the caller must suspend keypad scans and prevent every other interrupt,
thread, or bus user from accessing either pin while the backend exists. The
caller must also serialize the backend's operations, `handoff_to_keypad`, and
`Drop` against every writer to GPIOA DATA/DIR and PORTA input-enable/open-drain.
This includes writers that only modify unrelated pins: each backend operation
uses a full-register read-modify-write, so a concurrent write to another bit
could otherwise be lost. Complete the transaction, including STOP, before
calling `handoff_to_keypad` or dropping the backend. Handoff drives SDA high
and restores PA11 output mode with input-enable and open-drain disabled,
matching the board setup. This backend does not arbitrate ownership or restore
keypad scan state.

The backend requires an explicit `Delay1us` provider and forwards every delay
request from the bit-flow. It has no built-in delay implementation or calibrated
timing claim. Runtime startup wiring, electrical behavior, and physical timing
validation remain follow-up work; these tests do not establish bench behavior.

From the repository root:

```sh
cargo test --locked --test i2c_bitbang_flow
cargo clippy --locked --test i2c_bitbang_flow -- -D warnings
rustfmt --check --edition 2024 src/i2c_bitbang_flow.rs tests/i2c_bitbang_flow.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/i2c_bitbang_flow.rs -o /tmp/i2c_bitbang_flow.rlib
```

The host tests check ordered edge and delay traces, mixed byte patterns, early
and 255th-sample ACK, timeout, PA11 restoration, read ACK/NACK, buffer
boundaries, exact sample counts, GPIO register traces and bit preservation, and
a startup settings read through the EEPROM bridge using mock registers and
delay. The isolated Cortex-M0 compile checks `no_std` compatibility, not
physical bus behavior.
