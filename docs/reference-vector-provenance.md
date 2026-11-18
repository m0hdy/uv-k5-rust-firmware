# Reference-vector provenance

## Source and capture

The outputs in [`tests/reference-vectors/golden.json`](../tests/reference-vectors/golden.json) were generated from the optional C reference at commit
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/commit/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
The capture code is independently authored. It compiles the reference's
`dcs.c` and `frequencies.c`, extracts `RADIO_CheckValidChannel` and
`SETTINGS_SaveSettings` to a temporary file for compilation against independent
host mocks, and executes the reference's `fw-pack.py`. It does not vendor or
modify the reference. The temporary extracted functions and compiled artifacts
are deleted after capture.

| Output category | Pinned source and generator | Cases and output |
| --- | --- | --- |
| CTCSS and DCS | [`dcs.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/dcs.c#L18-L113): option arrays and `DCS_GetGolayCodeWord`, `DCS_GetCdcssCode`, and `DCS_GetCtcssCode`. | First/last tone, out-of-range tones, first/last DCS option, reverse word, decoded word and invalid word. Exact integer inputs and results are in the fixture. |
| Receive/transmit range and tuning | [`frequencies.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/frequencies.c#L19-L239): band tables, rounding and frequency checks. | Wide-RX chip bounds and gap, TX lock-mode boundaries, and normal/8.33-kHz rounding cases. Frequency values use the C source's 10-Hz ticks. `ENABLE_WIDE_RX` is enabled. |
| Channel validation | [`radio.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/radio.c#L59-L80): `RADIO_CheckValidChannel`; [`misc.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/misc.h#L39-L52): channel ranges. | First/last memory, first frequency, last NOAA, uint16 maximum, invalid band, scan-list 1/2 membership, both priority exclusions, and unsupported scan-list index. Inputs/state are independently assigned by `capture_channel()` in `capture.c`. |
| EEPROM settings encoding | [`settings.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/settings.c#L470-L595): `SETTINGS_SaveSettings`. | Standard and boundary profiles capture the ten 8-byte write blocks at their EEPROM offsets. A host mock records the writes; no physical EEPROM is accessed. Feature defines follow the pinned Makefile defaults for `ENABLE_WIDE_RX`, `ENABLE_VOX`, `ENABLE_DTMF_CALLING`, `ENABLE_RSSI_BAR`, `ENABLE_AUDIO_BAR`, and `ENABLE_AM_FIX`, as defined in the [pinned Makefile](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/Makefile#L12-L37). Input assignments are in [`capture.c`](../tools/reference-vectors/capture.c). |
| Firmware container checksum | [`fw-pack.py`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/fw-pack.py#L20-L38). | The pinned script receives a 12,288-byte `00..ff` repeating payload, author `Golden`, version `1.0`; expected raw and packed SHA-256, packed length, and appended CRC bytes are recorded. This is the updater-container CRC, not a settings checksum. |

The fixture records computed outputs rather than copied code, tables, bitmaps,
or other upstream source data. The inspected C files carry Dual Tachyon
copyright and Apache-2.0 notices; the pinned repository root is Apache-2.0.
No upstream implementation or table is included in this repository. The
host data structures and test values in `settings_capture_support.h` and
`capture.c` are independently authored mock state and inputs; the harness
includes the pinned `misc.h`/`frequencies.h` definitions and extracts the
`TxLockModes_t` enum from pinned `settings.h` into its temporary build
directory; none of those upstream definitions are copied into the repository.
The EEPROM mock only accepts the ten expected 8-byte writes at their
documented offsets, rejects any unexpected write address or count before
copying its bytes, checks bounds and null data, and verifies the final count.

Settings EEPROM read validation is not captured: [`SETTINGS_InitEEPROM`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/settings.c#L41-L340)
maps persisted values to defaults or clamps according to field-specific
policies, including screen/memory/frequency-channel bounds, setting ranges,
scan-list state, TX-lock mode, and DTMF code validation. Exercising that load
path also pulls in UI, radio, and device helpers, so malformed EEPROM reads
and their defaults remain an explicit gap. No settings checksum routine was
identified in `SETTINGS_SaveSettings`; the firmware-container CRC fixture does
not test settings validation or a settings checksum.

## Regeneration and test

Use a clean checkout at the pinned revision. The capture command rejects a
different `HEAD` or any tracked/untracked working-tree changes before compiling
or running code:

```sh
python3 -m pip install crcmod==1.7
UV_K5_C_REFERENCE=/path/to/clean/uv-k5-firmware-custom \
  python3 tools/reference-vectors/capture.py --write
UV_K5_C_REFERENCE=/path/to/clean/uv-k5-firmware-custom \
  python3 -m unittest discover -s tests/reference-vectors -v
```

The capture uses a host C compiler supporting GNU C2x (`cc` by default, or
`CC`), Python 3, and the pinned packer's `crcmod` dependency. The recorded
capture used GCC 13.3.0 and Clang 18.1.3, Python 3.12.3, and `crcmod` 1.7.
Both compilers reproduced the same fixture. These host tools are for reference
capture only and are not Rust firmware build prerequisites.

## Evidence and limits

The reference source files used for capture were independently checked
against the pinned Git object IDs: `dcs.c` `4615dc5136cd6e5d3ee84617c2781fa4d49f1417`,
`frequencies.c` `df3a193fef818bac9c8f083ea4cd360e9f06a466`,
`settings.c` `ce037002cf486ea93e7d5a982bce7e1e1f685515`, and
`radio.c` `a7db68d487ba6eff79ac23d0df36c443d8708741`,
`fw-pack.py` `4200248f3efaa6dec950ebb57678a241006872ef`. The corresponding
header blob IDs are `dcs.h` `873ddb6af636b36b146fb2fa3b15245d6d7e9a9d`,
`frequencies.h` `b5d4c16fbbcc06bccc3b92536fbe863aae54dfdd`,
`misc.h` `2ea627debdb3ecaa1304fe98a0908566c198fb40`, and
`settings.h` `16b9ac01f6b8a12b3d3265cf186b630b580fe760`.
The numeric fixture inputs are independently selected cases; each expected
value is emitted by the referenced function or packer, not copied from an
upstream vector table.
The updated 39-case fixture has SHA-256
`361172955485ba9d7b6cd3b26cafc0f73cc8ed9e56ee84d79c36b8da6049c748`.

This repository has no firmware CI workflow. The separate default C build
evidence is recorded in [issue #11's evidence comment](https://github.com/m0hdy/uv-k5-rust-firmware/issues/11#issuecomment-6982758020);
it is not a CI run of these vectors. In this sandbox, individual files were
read from the pinned revision and their Git object IDs verified. Captures with
GCC and Clang both matched the fixture. A full clean upstream checkout was
unavailable, so the capture's clean-checkout gate and the unittest command
above were not run here. The harness requires that gate for future
regeneration; this limitation is not a pass.

No firmware image was built or flashed for this capture. Physical-device
results are inapplicable because the work only records deterministic outputs
from source functions and uses a host EEPROM mock; device/recovery validation
remains tracked separately in [issue #12](https://github.com/m0hdy/uv-k5-rust-firmware/issues/12).
