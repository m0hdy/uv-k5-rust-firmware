# SPI0 startup register adapter

`src/spi0_startup_adapter.rs` adapts the pinned SPI0 initialization register
sequence to a typed register-I/O boundary. It is standalone and is not called
by firmware startup. It accesses only SPI0 CR, SPI0 IE, and
`SYSCON_DEV_CLK_GATE`.

## Ordered initialization

The sequence follows
[`driver/spi.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/spi.c#L23-L44)
at pinned revision
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3):

1. Read CR and write it with SPE cleared.
2. Read `SYSCON_DEV_CLK_GATE` and write it with only the SPI0 gate bit set.
3. Read CR again and write it with SPE cleared.
4. Read CR again, apply the SPI0 configuration, and write CR.
5. Replace IE with zero, then read IE as the source does.
6. Read CR again and write it with SPE set.

Each CR disable/configure/enable operation uses its own fresh read. The
configuration retains unrelated CR bits, including `TF_CLR`, while setting
the configured SPI fields. After the IE zero replacement, a nonzero readback
is intentionally ignored: this adapter does not enable or disable NVIC lines,
and does not retry the IE write. Tests cover both the zero readback and this
unexpected-value policy.

| Register | Address |
| --- | ---: |
| SPI0 CR | `0x400b8000` |
| SPI0 IE | `0x400b8010` |
| SYSCON device clock gate | `0x40000008` |

## Target preconditions and scope

The target-only volatile entry is available only for ARM bare-metal targets
and uses aligned 32-bit volatile register accesses. Before calling it, the
caller must ensure:

* The target is a DP32G030 with the register map listed above.
* SPI0 and SYSCON are accessible before entry. In particular, the SPI0 clock
  gate must already be enabled because the sequence accesses CR before its
  clock-gate read-modify-write.
* SPI0 display pins B7 SSN, B8 CLK, and B10 MOSI are already muxed.
* This call has exclusive ownership of SPI0 CR, SPI0 IE, and
  `SYSCON_DEV_CLK_GATE` for the whole sequence.

The adapter does not configure pins, enable an initially-disabled SPI0 clock,
perform NVIC operations, connect to display startup, or claim device-level
validation.

Host tests record and verify the literal ordered accesses without touching a
device. Focused validation from the repository root:

```sh
cargo test --locked --test spi0_startup_adapter --test spi0_plan
cargo clippy --locked --test spi0_startup_adapter -- -D warnings
rustfmt --check --edition 2024 \
  src/spi0_plan.rs src/spi0_startup_adapter.rs tests/spi0_startup_adapter.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings -C opt-level=2 -C link-dead-code=yes --emit=asm \
  src/spi0_startup_adapter.rs -o /tmp/spi0_startup_adapter.s
```

Inspect the retained `initialize_spi0` assembly for volatile word loads and
stores at the three addresses above and their source order. The assembly is
temporary inspection evidence, not executed or published. These checks do not
establish device behavior.
