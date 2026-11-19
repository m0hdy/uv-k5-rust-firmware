# Core settings startup defaults provenance

The standalone decoder in
[`src/settings_boot_defaults.rs`](../src/settings_boot_defaults.rs) models
only the eight bytes at EEPROM address `0x0E70` in
[`SETTINGS_InitEEPROM`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/settings.c#L41-L57)
from `egzumer/uv-k5-firmware-custom` commit
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/commit/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
`IS_MR_CHANNEL` and `MR_CHANNEL_FIRST`/`MR_CHANNEL_LAST` come from pinned
[`misc.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/misc.h#L39-L52).

| Byte | Field | Accepted values | Invalid value default |
| --- | --- | --- | --- |
| 0 | Call channel | `0..=199` | `0` |
| 1 | Squelch level | `< 10` | `1` |
| 2 | TX timeout timer | `< 11` | `1` |
| 3 | NOAA auto scan (`ENABLE_NOAA`) | `< 2` | `false` |
| 4 | Key lock | `< 2` | `false` |
| 5 | VOX switch (`ENABLE_VOX`) | `< 2` | `false` |
| 6 | VOX level (`ENABLE_VOX`) | `< 10` | `1` |
| 7 | Microphone sensitivity | `< 5` | `4` |

When `ENABLE_NOAA` or `ENABLE_VOX` is absent, its result fields are `None`.
They are absent from this model in that build; `None` does not mean the bytes
are written, cleared, or reset. The feature-presence argument allows all four
independent NOAA/VOX combinations to be tested.

The tests compare literal decoded fields against the assignments in the pinned
source for valid bounds, each first invalid value, erased `0xff` data, and all
feature-presence combinations. This is a pure software model, not an EEPROM
reader or writer, firmware-startup integration, device test, or broader
settings validation claim.
