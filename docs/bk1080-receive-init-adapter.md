# BK1080 nonzero-frequency initialization

`src/bk1080_receive_init_adapter.rs` executes the nonzero-frequency
`BK1080_Init` branch from
[`driver/bk1080.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/bk1080.c)
at pinned reference
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
It is a standalone adapter: it is not exported from the library or connected
to board startup, application startup, or a radio enable path.
Zero frequency is rejected without touching GPIO or the bus; the separate
zero-frequency shutdown adapter remains responsible for that branch.

The 34 table values were independently checked in order against
`BK1080_RegisterTable` in the pinned C source. The adapter reuses the register
framing and `set_frequency` operation in
[`bk1080_receive_adapter.rs`](bk1080-receive-adapter.md); it does not duplicate
the I2C or tuning protocol.

## Initialization and state

The caller owns an `Initialized` value; there is no hidden global state. Start
with `Initialized::new()`. On a call with `is_initialized() == false`, the
adapter clears PB15 using a fresh GPIOB DATA read-modify-write, sends the
34-register table to register indices 0 through 33, waits 250 ms, writes
register index 25 (`0x19`) with `0xA83C` and then `0xA8BC`, waits 60 ms, and
only then marks calibration complete. It subsequently writes register 5 with
`0x0A1F` and calls the existing frequency tuner, including its fresh register
5 read/modify/write and 10 ms tune delay.

When the caller's state is already initialized, the adapter clears PB15,
skips the table and calibration delays, writes register 2 with `0x0201`, then
performs the same register 5 write and frequency tuning. A normal receiver
shutdown via the zero-frequency branch does not invalidate this state. Keep
the value alive across ordinary shutdown and warm enable. Replace it with
`Initialized::new()` after hardware reset, power loss, or any other event that
may have lost the receiver's calibration/register state.

Calibration is not recorded as complete until every calibration write and
delay has finished. A failed operation before that point leaves the state
false, so a later caller-controlled invocation can retry the full table.
Once calibration succeeds, a later configuration or tuning failure leaves the
state true; the next invocation uses the warm-enable path. There are no
automatic retries.

## Errors and GPIO behavior

Unlike the pinned C driver, which ignores I2C acknowledgements, this Rust
executor returns the first transport or NACK error. A failed START is returned
without STOP. A failed byte or read triggers one best-effort STOP; its error
does not replace the original bus error. A failed final STOP is returned
without retry. No later write or delay runs after a failed bus operation. On
any error, a fresh GPIOB DATA read-modify-write asserts PB15 after transaction
cleanup; the original bus error is retained and unrelated GPIOB bits are
preserved. The first calibration remains incomplete after any table or
calibration failure.

`initialize_with_io` provides the testable executor over the existing
`I2cTransport`, `WordIo`, and `DelayMs` interfaces. The ARM bare-metal-only
`initialize_mmio` convenience constructs the existing GPIOA10/11 bitbang owner,
uses volatile GPIOB DATA access, and hands the keypad pins back on both success
and error. It takes explicit one-microsecond and millisecond delay providers.
They must accurately implement each requested interval (bitbang 1 us delays,
250 ms, 60 ms, and the tuning operation's 10 ms), be infallible and
non-panicking, and not access registers shared with the operation.

## Target ownership

The unsafe target caller must provide mapped DP32G030 GPIOA, GPIOB, and
PORTCON registers; GPIOA and GPIOB clocks must be enabled; PB15 must be
configured as an output; and PA10/PA11 must be muxed to GPIOA10/GPIOA11 and
configured as GPIO outputs with input-enable and open-drain disabled on both
pins. PA10/PA11
are shared with the keypad. The caller must exclusively own the BK1080 I2C bus,
PB15, and keypad-shared pins for the full operation, including delays,
transport cleanup, handoff, and drop. Serialize against interrupts and every
writer to GPIOA DATA/DIR, PORTA input-enable/open-drain, and GPIOB DATA.
Unrelated-bit writers must be excluded too because GPIO changes use
whole-register read-modify-writes. Handoff restores the board's keypad pin
configuration but does not resume keypad scanning or arbitrate ownership.

## Focused validation

From the repository root:

```sh
cargo test --locked --test bk1080_receive_init_adapter
cargo clippy --locked --test bk1080_receive_init_adapter -- -D warnings
cargo fmt --all -- --check

ROOT="$(pwd)"
RUSTFLAGS='-D warnings' cargo build --locked --lib --target thumbv6m-none-eabi
HARNESS="$(mktemp -d /tmp/bk1080-receive-init-target-XXXXXX)"
cat > "$HARNESS/lib.rs" <<EOF
#![no_std]
pub use uv_k5_rust_firmware::{eeprom_i2c_bridge, eeprom_read_transport, i2c_bitbang_flow};
#[path = "$ROOT/src/bk1080_receive_init_adapter.rs"]
pub mod bk1080_receive_init_adapter;
EOF
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi -D warnings \
  --extern "uv_k5_rust_firmware=$ROOT/target/thumbv6m-none-eabi/debug/libuv_k5_rust_firmware.rlib" \
  "$HARNESS/lib.rs" -o "$HARNESS/libbk1080_receive_init_adapter.rlib"
```

The focused host traces cover the complete first-init and warm-enable paths,
the exact table and delays, preserving PB15 updates, and failures before and
after calibration. The standalone harness compiles the unexported target-only
MMIO entry point against the real library modules. No radio, audio, or device
validation is claimed; there is no flashing, bootloader work, device
execution, installable release, or application enable integration in this
change.
