# Offline board startup plan composition

`src/board_startup_composition.rs` composes the existing call sequencer with
the PortCon SEL, PortCon input/pull, and GPIO direction/data plans. The
composition is isolated from `src/lib.rs` and firmware startup. It records
typed plan writes and mock call events; it performs no MMIO or device access.

## Pinned provenance

The composition follows
[`egzumer/uv-k5-firmware-custom@7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3):

- [`main.c:61–77`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/main.c#L61-L77)
  enables clock gates, calls `SYSTICK_Init()`, then calls `BOARD_Init()`.
- [`board.c:61–111`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/board.c#L61-L111)
  contains the GPIO direction and optional FM data-level writes.
- [`board.c:116–455`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/board.c#L116-L455)
  contains the SEL writes followed by input, pull, and open-drain writes.
- [`board.c:490–505`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/board.c#L490-L505)
  calls PortCon, GPIO, backlight, ADC, and display in that order, then
  conditionally initializes FM and CRC.

The composer feeds the sequencer's `PortconInit` phase into the existing SEL
plan followed by the existing input/pull plan. It feeds `GpioInit` into the
existing GPIO plan. These three phases appear as plan-write events rather than
duplicate call markers; all other sequencer calls remain typed mock events.
`ENABLE_SWD` is supplied to both PortCon plans. `ENABLE_FMRADIO` is supplied
to the GPIO plan and remains a sequencer define for the optional BK1080 call.

## Validation and scope

Host tests use distinct caller-supplied snapshots and exercise SWD and FM
feature combinations. They compare composed writes with each existing plan,
check the combined event order, and check that GPIO application leaves bits
outside the plan's update masks unchanged.

```sh
cargo test --locked --test board_startup_composition
cargo clippy --locked --test board_startup_composition -- -D warnings
rustfmt --check --edition 2024 \
  src/board_startup_composition.rs tests/board_startup_composition.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/board_startup_composition.rs \
  -o /tmp/board_startup_composition.rlib
```

The standalone target compile checks that the composition needs neither `std`
nor an allocator. This software-only child task does not add a public library
API, hardware backend, firmware startup change, RF transmission, release,
upload, or device testing; it does not establish hardware acceptance.
