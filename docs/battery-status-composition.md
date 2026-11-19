# Offline battery status composition

[`src/battery_status_composition.rs`](../src/battery_status_composition.rs)
composes the battery status text transformation and indicator composition
without duplicating their formatting or fill rules. It follows
[`ui/status.c` lines 146–183](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/ui/status.c#L146-L183)
at pinned revision
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).

The render plan emits visible text first, an optional caller-supplied USB-C
pattern at column 102 while charging, then the 17-byte battery bitmap at column
111 regardless of charging. Text placement and omission are delegated to
[`battery_status_text.rs`](../src/battery_status_text.rs); bitmap fill and blink
behavior are delegated to [`battery_indicator.rs`](../src/battery_indicator.rs).
The bitmap operation always contains all 17 columns, including when blink
blanking makes them zero. No USB operation is emitted while not charging, and
omitting text does not affect either icon operation.

This is standalone `no_std` code. Tests use synthetic USB, outline, and bar
patterns with a host-side operation capture; there is no font renderer, source
bitmap asset, display I/O, blit wiring, MMIO, or hardware validation.

Run focused checks from the repository root:

```sh
cargo test --locked --test battery_status_composition
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/battery_status_composition.rs -o /tmp/battery_status_composition.rlib
```

The test captures ordered text, charging icon, and battery operations; checks
the nine-column text shift; and compares all battery bitmap bytes for filled
and blink-blanked cases.
