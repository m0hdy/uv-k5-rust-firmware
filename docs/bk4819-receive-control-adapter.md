# BK4819 receive power-control adapter

[`src/bk4819_receive_control_adapter.rs`](../src/bk4819_receive_control_adapter.rs)
provides caller-driven `receive_on`, `sleep`, and `idle` operations. The
standalone adapter reuses
[`src/bk4819_register_transport.rs`](../src/bk4819_register_transport.rs)
for every register write and requires a caller-supplied, accurate,
non-panicking one-microsecond delay. It is not declared in `src/lib.rs` and is
not wired into radio or application policy.

The sequence and values match `BK4819_RX_TurnOn`, `BK4819_Sleep`, and
`BK4819_Idle` in the pinned
[`driver/bk4819.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/bk4819.c#L792-L1090).
Register-30 fields are from the pinned
[`driver/bk4819-regs.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/bk4819-regs.h#L230-L272).

| Operation | Ordered register writes |
| --- | --- |
| Receive on | `37=1F0F`, `30=0000`, `30=BFF1` |
| Sleep | `30=0000`, `37=1D00` |
| Idle | `30=0000` |

`BFF1` is independently composed from the pinned register-30 enables:
VCO calibration `8000`, RX link `3C00`, AF DAC `0200`, discriminator
`0100`, PLL/VCO `00F0`, and RX DSP `0001`. Their bitwise OR is `BFF1`.
Unknown `4000`, PA gain `0008`, MIC ADC `0004`, and TX DSP `0002` are not
enabled. These operations perform no BK4819 chip/register reads, ACKs, extra
waits, retries, GPIO-output shadow changes, idle-mode state changes, or radio
policy. GPIO register reads used by the existing transport's
read-modify-write operations remain part of each serial write.

## Target MMIO entry points

For `arm`/`none`, `receive_on_mmio`, `sleep_mmio`, and `idle_mmio` are unsafe
entries that route every write through the existing volatile GPIOC/PORTCON
implementation. Each takes the caller's `Delay1us` provider. They do not
initialize the radio, clocks, pins, frequency, or RF policy.

Before any target entry, the caller must ensure the BK4819 is initialized and
the DP32G030 GPIOC and PORTCON mappings are present and accessible, the GPIOC
clock is enabled, and GPIOC0/1/2 are muxed to GPIO, configured as outputs, and
have input-enable and open-drain disabled. The caller must exclusively own the
BK4819 GPIO bus for the entire call and serialize it against every writer to
GPIOC DATA/DIR and PORTC input-enable/open-drain, including interrupts and
writers to unrelated bits. The transport performs full-word
read-modify-write accesses and provides no locking. The delay provider must
implement accurate, non-panicking one-microsecond delays.

## Validation and scope

The focused host tests decode the complete GPIO DATA serial traces for all
three operations, including every GPIO DATA read/write and transport delay.
The warning-denied Cortex-M0 fixture instantiates all three volatile entries
with an external caller-delay symbol; it is compiled only and never executed.
Host traces do not establish device, electrical, timing, receive, or RF
behavior.

From the repository root:

```sh
cargo test --locked --test bk4819_receive_control_adapter
cargo clippy --locked --test bk4819_receive_control_adapter -- -D warnings
rustfmt --check --edition 2024 \
  src/bk4819_receive_control_adapter.rs \
  tests/bk4819_receive_control_adapter.rs \
  tests/fixtures/bk4819_receive_control_target.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings tests/fixtures/bk4819_receive_control_target.rs \
  -o /tmp/bk4819_receive_control_target.rlib
```

Tuning, transmit support, squelch/audio policy, application integration,
device execution, and bench acceptance remain out of scope under
`m0hdy/uv-k5-rust-firmware#45`.
