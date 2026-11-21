# Keyboard GPIO adapter

[`src/keyboard_gpio_adapter.rs`](../src/keyboard_gpio_adapter.rs) connects
the retained [`KeyboardPollFlow`](../src/keyboard_poll_flow.rs) and its
`MatrixIo` contract to GPIOA DATA and the existing I2C STOP flow. The adapter
owns one register backend and one delay provider. It does not create an
owning `GpioI2cIo`; STOP uses a short-lived `I2cIo` borrow of the same GPIO
backend, so STOP cannot run a second backend's `Drop` cleanup.

## Poll behavior and register access

The caller retains one `KeyboardGpioAdapter` and calls `poll` once every 10 ms
to reproduce pinned application key timing
([`app/app.c:1022–1085`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/app/app.c#L1022-L1085);
see also the [keyboard poll flow](keyboard-poll-flow.md)). Debounce, hold, and
repeat thresholds count calls, not elapsed time, so missed or extra calls
change wall-clock key timings. The 1 µs delay only settles GPIO/bus edges; it
does not schedule polls. This adapter has no scheduler or cadence source, so
the caller owns scheduling. It delegates to the existing
`KeyboardPollFlow::poll`, preserving its retained debounce decisions, event
ordering, and mapping. The existing matrix scanner is unchanged: it checks
side keys first, then the PA10–PA13 rows; each row's select/update is a
separate full-register operation, each 1 µs delay precedes its GPIOA DATA
sample, and each sample is read from the complete 32-bit DATA register before
the scanner compares its low 16 bits. Every normal scan
result—key, no key, or unstable/noise abort—runs the shared STOP followed by
clear-PA12 and set-PA13 cleanup.

Every GPIOA DATA update reads the latest complete register value and writes
the masked result; matrix row masks are widened to preserve bits 16–31 while
retaining their exact low-16-bit selection. The pinned C poll clears those
upper bits because its masks are 16-bit; preserving them here is an intentional
register-safety divergence, with low-16-bit row selection and key priority
unchanged. STOP applies the same fresh read-modify-write policy to GPIOA DATA.
No update combines adjacent operations. This preserves unrelated bits in each
operation but does not make concurrent register writers safe.
STOP's pin order and delays come from
[`i2c_bitbang_flow::stop`](../src/i2c_bitbang_flow.rs), not a duplicate local
sequence.

## Target preconditions and exclusive ownership

The target constructor is unsafe and only available for ARM bare-metal
targets. Before constructing the adapter, board setup must:

- enable the GPIOA clock and select GPIO function for PA3–PA6 and PA10–PA13;
- configure PA3–PA6 as inputs and PA10–PA13 as outputs in GPIOA DIR;
- configure PORTA input-enable, pull, and open-drain state for the keypad;
  PA10/PA11 must be outputs with input-enable and open-drain disabled.

The expected pin and direction setup follows the pinned `BOARD_GPIO_Init` and
`BOARD_PORTCON_Init` contracts; this adapter does not configure clocks, mux,
direction, pulls, or open-drain state.

From construction through the complete poll, the caller must give the adapter
exclusive keypad/I2C/voice ownership of PA3–PA6 and PA10–PA13. Serialize polls
against all other accesses to these pins and against every writer to GPIOA
DATA/DIR and PORTA input-enable/open-drain, including interrupt handlers and
writers that only change unrelated bits. The backend performs full-register
read-modify-write operations, so even unrelated concurrent writes can be lost.
The adapter provides no lock, interrupt masking, arbitration, or handoff
protocol.

The supplied `Delay1us` provider must accurately implement each requested
microsecond, must be infallible, and must not access the shared GPIO/PORTA
registers. Register and delay traits are infallible, so normal scan outcomes
reach STOP and voice cleanup. A panic or abort during a backend operation or
delay does not guarantee cleanup; the target panic strategy and hardware
execution are outside this module's contract.

## Scope and validation

The adapter is exported from `src/lib.rs` and the opt-in firmware entry polls
it from the scheduler's 10 ms hook. Its target use assumes the startup keypad
handoff and the exclusive GPIOA contract above. PTT and the full application
event policy are not wired. Host register trace tests do not execute MMIO or
validate electrical behavior, device timing, or board revisions.

From the repository root:

```sh
cargo test --locked --test keyboard_gpio_adapter
cargo clippy --locked --test keyboard_gpio_adapter -- -D warnings
rustfmt --check --edition 2024 \
  src/keyboard_gpio_adapter.rs tests/keyboard_gpio_adapter.rs \
  tests/fixtures/keyboard_gpio_adapter_target.rs
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings tests/fixtures/keyboard_gpio_adapter_target.rs \
  -o /tmp/keyboard_gpio_adapter_target.rlib
```

The target fixture compiles the production modules without `cfg(test)` exports
or hardware execution. Host tests use `RegisterIo::read` to supply and record
the actual scan samples; they verify key priority, all row/column mappings,
zero and boundary samples, no-key/noise cleanup, and retained debounce across
direct key changes.
