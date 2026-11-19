# BK4819 GPIO register transport

[`src/bk4819_register_transport.rs`](../src/bk4819_register_transport.rs)
executes the pinned BK4819 GPIO serial register reads and writes as a
standalone, allocation-free adapter. It is intentionally not declared in
`src/lib.rs`, is not connected to application startup, and performs no I/O
when the module or its traits are constructed. Callers provide both typed
32-bit word I/O and an explicit one-microsecond delay provider.

The sequence follows `BK4819_ReadU16`, `BK4819_ReadRegister`,
`BK4819_WriteRegister`, `BK4819_WriteU8`, and `BK4819_WriteU16` in
[`driver/bk4819.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/bk4819.c#L127-L240),
and the SCN/SCL/SDA pin assignments in
[`driver/gpio.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/gpio.h#L52-L54),
at pinned revision
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
It is the chip's own GPIO serial protocol, not EEPROM/BK1080 I2C or a guessed
SPI transaction. Register addresses remain raw `u8` values; reads send
`address | 0x80`, writes send the address unchanged, and both transfer the
address MSB-first followed by the 16-bit value MSB-first.

| Word | Address | Fields used |
| --- | ---: | --- |
| GPIOC DATA | `0x40061000` | SCN bit 0, SCL bit 1, SDA bit 2 |
| GPIOC DIR | `0x40061004` | SDA bit 2 |
| PORTC input-enable | `0x400b0108` | SDA bit 2 |

These addresses derive from `GPIOC` at `0x40061000` in pinned
[`hardware/dp32g030/gpio.def`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/hardware/dp32g030/gpio.def)
and `PORTCON` at `0x400b0000` plus `PORTC_IE` offset `0x0108` in pinned
[`hardware/dp32g030/portcon.def`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/hardware/dp32g030/portcon.def).
Each GPIO or input-enable bit update reads the latest complete word and writes
the updated word, preserving unrelated bits without coalescing redundant
operations.

Reads retain the original turnaround: enable C2 input, switch C2 to input,
wait 1 us, sample SDA before each of sixteen rising SCL edges, then disable
input-enable before restoring output direction. The final state is SCN, SCL,
and SDA high, with SDA input-enable disabled and output direction restored.
Every C delay call is forwarded once to the supplied provider. No ACK,
repeated-start, timeout, retry, clamping, or RF policy is added.

## Target MMIO prerequisites and ownership

The volatile target entry points exist only for `arm`/`none` and are unsafe.
The caller must ensure the DP32G030 GPIOC and PORTCON mappings are present,
GPIOC's SYSCON device-clock gate is enabled (pinned `DEV_CLK_GATE` at
`0x40000008`, GPIOC bit 2), and GPIOC0/1/2 are muxed to GPIO (`PORTC_SEL0`
fields at `0x400b0010` set to GPIOC0/1/2). Before entry, all three pins must
be outputs with input-enable and open-drain disabled. These are the relevant
states set by pinned `BOARD_GPIO_Init` and
`BOARD_PORTCON_Init` in
[`board.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/board.c).
The read temporarily enables C2 input and changes its direction, then restores
the stated setup.

The caller must own SCN/SCL/SDA for the entire transaction and serialize each
entry against every writer to GPIOC DATA/DIR and PORTC input-enable, including
interrupts and writers that only touch unrelated bits. The backend performs
full-word read-modify-write operations and does not provide locking. The
explicit delay provider must implement accurate, non-panicking 1-us delays;
there is no timing default. These functions do not configure pins, enable
clocks, arbitrate ownership, initialize the chip, or establish electrical or
device-level behavior.

## Scope and validation

This is the register transport slice only. Initialization/control operations
and application integration remain for follow-up work under
`m0hdy/uv-k5-rust-firmware#45`. No receive/transmit behavior, RF operation,
device execution, bench acceptance, or parent-issue completion is claimed.

From the repository root:

```sh
cargo test --locked --test bk4819_register_transport
cargo clippy --locked --test bk4819_register_transport -- -D warnings
rustfmt --check --edition 2024 \
  src/bk4819_register_transport.rs tests/bk4819_register_transport.rs \
  tests/fixtures/bk4819_register_transport_target.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings tests/fixtures/bk4819_register_transport_target.rs \
  -o /tmp/bk4819_register_transport_target.rlib
```

The focused host tests compare complete successful GPIO, direction,
input-enable, and delay traces to an independent reference trace, including
asymmetric addresses and values, zero/all-one payloads, the read turnaround,
idle levels, and preservation across changing register snapshots. The
warning-denied Cortex-M0 fixture imports and instantiates both unsafe volatile
entry points using an external delay symbol; it is compiled only and never
executed. None of these checks accesses a device.
