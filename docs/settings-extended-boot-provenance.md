# Extended settings startup provenance

`src/settings_extended_boot.rs` models the portable decoding in
[`SETTINGS_InitEEPROM`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/settings.c#L105-L270)
at pinned revision `7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`. It contains no
EEPROM access and performs no hardware action.

| EEPROM offset | Rust decoder | Compatibility details |
| --- | --- | --- |
| `0x0E90` | `decode_key_and_scan` | C `uint8_t` fields and packed action byte are preserved. Invalid key actions fall back to the exact pinned action constants; scan resume falls back to `SCAN_RESUME_CO` (`1`). |
| `0x0E98` | `decode_power_on_password` | Copies the first four bytes exactly, matching the C `memcpy` into its 32-bit field on the target's little-endian layout. |
| `0x0EA0` | `decode_voice_and_rssi` | Voice and RSSI values remain absent when their build features are disabled. The RSSI bounds and `(130, 76)` fallback match the C conditions. |
| `0x0EA8` | `decode_alarm`, `decode_roger_and_transmitter` | Optional alarm state is feature-gated; invalid alarm, roger, repeater-tail, TX VFO and battery-type fields use the C fallback values. |
| `0x0ED0`, `0x0ED8` | `decode_dtmf` | DTMF calling fields are represented as `Option` when the feature is absent. Timing uses explicit `u16` tenths-of-10-ms multiplication and the per-field C fallbacks. |
| `0x0F18` | `decode_scan_lists` | Enable bytes use the C `< 2` test; priority channel bytes are retained without validation. |
| `0x0F40` | `decode_global` | Bitfields and invalid enum fallbacks match the pinned code. Build-specific killed, mic-bar, and AM-fix values remain `Option`. |

This decoder does not claim to implement `SETTINGS_InitEEPROM` as a whole.
The optional `0x0E88` FM configuration and `0x0E40` FM channel data, DTMF
calling strings at `0x0EE0..0x0F17`, `0x0F30` custom AES key, channel
attributes, target-entry wiring, and calibration loading remain separate
work. `settings_channel_acquisition::acquire_full_portable_with_transport`
reads the existing `0x0E70`, `0x0E78`, and `0x0E80` sequence followed by the
eight offsets above, stopping at the first I/O error. The optional FM read is
not issued by this API. These fields are not silently treated as validated or
as checksummed. The pinned settings code does not define an EEPROM settings
checksum; the firmware-container CRC is unrelated.
