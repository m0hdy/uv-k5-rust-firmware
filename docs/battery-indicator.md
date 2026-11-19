# Offline battery indicator composition

`src/battery_indicator.rs` models `UI_DrawBattery()` from
[`ui/battery.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/ui/battery.c#L26-L48)
at pinned revision
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
The matching [`bitmaps.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bitmaps.h)
declares a 17-byte `BITMAP_BatteryLevel1` outline and a 2-byte
`BITMAP_BatteryLevel` bar. The normal fill begins at offsets 13, 10, 7, and 4;
the reversed fill begins at 3, 6, 9, and 12, following the source expressions
`sizeof(BITMAP_BatteryLevel1) - 4 - (i * 3)` and `3 + (i * 3)`.

The transformation uses caller-supplied patterns, blanks only when `level < 2`
and `blink == 1`, copies the outline for other non-filled cases, and fills at
most four bars. `FillDirection` represents the source's
`ENABLE_REVERSE_BAT_SYMBOL` compile-time selection. The module is allocation
free and has no display, ADC, startup, or MMIO wiring; it does not copy or
provide upstream bitmap assets.

Focused checks from the repository root:

```sh
cargo test --locked --test battery_indicator
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/battery_indicator.rs -o /tmp/battery_indicator.rlib
```

The host tests use synthetic patterns and compare all 17 output bytes for
blanking, outline-only cases, every fill count in both directions, saturation,
and non-boolean blink values. The direct target compile checks this standalone
module with warnings denied; it does not link it into the firmware.
