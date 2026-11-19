# GPIO startup register adapter

`src/gpio_startup_adapter.rs` applies the typed updates from
[`gpio_init_plan.rs`](../src/gpio_init_plan.rs) through a small register-I/O
boundary. Its address type names only the six GPIOA/B/C DATA and DIR registers;
there is no arbitrary-address volatile API. The module is not included from
`src/lib.rs` or firmware startup.

## Address map and update semantics

At pinned upstream revision
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3),
[`bsp/dp32g030/gpio.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bsp/dp32g030/gpio.h#L20-L35)
defines GPIOA/B/C base addresses and a `GPIO_Bank_t` with consecutive 32-bit
`DATA` and `DIR` fields. Thus each bank's DIR address is its base plus four.
[`board.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/board.c#L61-L111)
expresses direction changes as separate `|=` and `&= ~` operations, retaining
the other register bits. The plan applies each update as one register read,
one set/clear calculation, and one write; a later update reads the latest
register value again.

| Port | DATA | DIR |
| --- | --- | --- |
| A | `0x40060000` | `0x40060004` |
| B | `0x40060800` | `0x40060804` |
| C | `0x40061000` | `0x40061004` |

These are the DP32G030 addresses declared by the pinned source; this adapter
does not independently validate a board revision or hardware mapping.

## Target preconditions and scope

The volatile backend is compiled only for ARM bare-metal targets and uses
32-bit `read_volatile`/`write_volatile` operations. Its public target entry is
`unsafe` because the caller must ensure the target is a DP32G030 using the
mapping above, enable the GPIOA/B/C peripheral clocks, and configure the
required pins for GPIO operation before applying the plan. No other context
may concurrently modify the affected DATA or DIR registers during the
read-modify-write sequence. The adapter does not enable clocks, configure pin
mux, initialize hardware, or call from startup.

Host tests use a fake register backend and do not access devices. Focused
validation from the repository root:

```sh
cargo test --locked --test gpio_startup_adapter
cargo clippy --locked --test gpio_startup_adapter -- -D warnings
rustfmt --check --edition 2024 \
  src/gpio_startup_adapter.rs tests/gpio_startup_adapter.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings --emit=asm src/gpio_startup_adapter.rs \
  -o /tmp/gpio_startup_adapter.s
```

The last command compiles the target-only adapter and emits assembly for
inspection of the volatile word accesses. The assembly is temporary evidence,
not executed or published. These checks do not establish device behavior.
