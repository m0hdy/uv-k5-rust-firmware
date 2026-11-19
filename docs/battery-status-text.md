# Offline battery status text

[`src/battery_status_text.rs`](../src/battery_status_text.rs) models battery
status text formatting and placement from
[`ui/status.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/ui/status.c#L146-L172)
at pinned revision
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
The placement constants follow [`driver/st7565.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/st7565.h#L27-L30)
(`LCD_WIDTH = 128`) and [`bitmaps.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bitmaps.h#L26-L33)
(`BITMAP_BatteryLevel1[17]`, `BITMAP_USB_C[9]`). The status code uses
7 pixels per byte when calculating text width.

Mode 1 formats hundredths of a volt as `d.ddV`, clamping values above 999 to
9.99V. Mode 2 formats the caller-supplied battery percentage as an integer
followed by `%`; the caller must provide an already converted value in
`0..=100`. Other modes produce no text. The text is right-aligned before the
battery icon, reserving USB icon width only while charging, and is omitted if
it would overlap the occupied prefix. An exact fit is accepted. The fixed
five-byte result is allocation-free and includes the start column only when
text is visible.

This is a standalone `no_std` transformation with no voltage conversion,
font or bitmap assets, display wiring, ADC, MMIO, or hardware validation.

Run focused checks from the repository root:

```sh
cargo test --locked --test battery_status_text
rustc --edition=2024 --crate-type=lib --target thumbv6m-none-eabi \
  -D warnings src/battery_status_text.rs -o /tmp/battery_status_text.rlib
```

The host tests cover voltage padding and clamping, representative percentage
values, disabled and unknown modes, charging-dependent placement, exact-fit and
one-pixel overlap, and an oversized occupied prefix.
