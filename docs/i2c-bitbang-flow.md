# Offline I2C bitbang flow

[`src/i2c_bitbang_flow.rs`](../src/i2c_bitbang_flow.rs) models
[`driver/i2c.c:23–147`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/i2c.c#L23-L147)
at pinned public revision
[`egzumer/uv-k5-firmware-custom@7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
It is a standalone, unexported, allocation-free `no_std` model. `I2cIo` supplies
infallible typed mock operations for SCL, SDA, PA11 input enable, open drain
and direction, SDA samples, and one-microsecond delays. These operations
represent the reference's register operations, not a hardware implementation.

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

PA10 (SCL) and PA11 (SDA) are shared with the keyboard. This model does not
establish hardware ownership, timing accuracy, or a clock-stretching policy;
it does not configure or access real GPIO/MMIO, integrate EEPROM, or change the
firmware image. No device execution is implied by these checks.

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
boundaries, and exact sample counts. The isolated Cortex-M0 compile checks
`no_std` compatibility, not physical bus behavior.
