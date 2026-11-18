# Reference-vector provenance

## Source and capture

The outputs in [`tests/reference-vectors/golden.json`](../tests/reference-vectors/golden.json) were generated from the optional C reference at commit
[`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/commit/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
The capture harness and its selected inputs are independently authored. It
compiles the reference's `dcs.c` and `frequencies.c`, extracts
`RADIO_CheckValidChannel` and `SETTINGS_SaveSettings` to a temporary file, and
executes the reference's `fw-pack.py`. The settings host mock is a minimized,
modified subset of the pinned `EEPROM_Config_t` declaration in `settings.h`;
its source notice and the Apache-2.0 license copy are in
[`settings_capture_support.h`](../tools/reference-vectors/settings_capture_support.h)
and [`LICENSE-APACHE-2.0`](../tools/reference-vectors/LICENSE-APACHE-2.0).
Other host state and test inputs are independently authored. The harness does
not vendor or modify the reference; temporary extracted functions and compiled
artifacts are deleted after capture.

| Output category | Pinned source and generator | Cases and output |
| --- | --- | --- |
| CTCSS and DCS | [`dcs.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/dcs.c#L18-L113): option arrays and `DCS_GetGolayCodeWord`, `DCS_GetCdcssCode`, and `DCS_GetCtcssCode`. | First/last tone, out-of-range tones, first/last DCS option, reverse word, decoded word and invalid word. Exact integer inputs and results are in the fixture. |
| Receive/transmit range and tuning | [`frequencies.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/frequencies.c#L19-L239): band tables, rounding and frequency checks. | Wide-RX chip bounds and gap, TX lock-mode boundaries, and normal/8.33-kHz rounding cases. Frequency values use the C source's 10-Hz ticks. `ENABLE_WIDE_RX` is enabled. |
| Channel validation | [`radio.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/radio.c#L59-L80): `RADIO_CheckValidChannel`; [`misc.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/misc.h#L39-L52): channel ranges. The Rust bridge calls [`channel_validity::is_valid_channel`](../src/channel_validity.rs). | First/last memory, first frequency, last NOAA, uint16 maximum, invalid band, scan-list 1/2 membership, both priority exclusions, and unsupported scan-list index. Inputs/state are independently assigned by `capture_channel()` in `capture.c`. |
| EEPROM settings encoding | [`settings.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/settings.c#L470-L595): `SETTINGS_SaveSettings`. | Standard and boundary profiles capture the ten 8-byte write blocks at their EEPROM offsets. A host mock records the writes; no physical EEPROM is accessed. Feature defines follow the pinned Makefile defaults for `ENABLE_WIDE_RX`, `ENABLE_VOX`, `ENABLE_DTMF_CALLING`, `ENABLE_RSSI_BAR`, `ENABLE_AUDIO_BAR`, and `ENABLE_AM_FIX`, as defined in the [pinned Makefile](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/Makefile#L12-L37). Input assignments are in [`capture.c`](../tools/reference-vectors/capture.c). |
| Firmware container checksum | [`fw-pack.py`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/fw-pack.py#L20-L38). | The pinned script receives a 12,288-byte `00..ff` repeating payload, author `Golden`, version `1.0`; expected raw and packed SHA-256, packed length, and appended CRC bytes are recorded. This is the updater-container CRC, not a settings checksum. |

The fixture's item-specific reuse decision is to retain this exact set of
independently selected inputs and computed outputs as behavioral test data.
It contains no copied implementation, table, bitmap, or upstream vector set.
Attribution for the generated results is the pinned
[`egzumer/uv-k5-firmware-custom`](https://github.com/egzumer/uv-k5-firmware-custom)
revision above; the inspected C sources identify Copyright 2023 Dual Tachyon
and Apache-2.0, and the repository root is Apache-2.0. The inspected
`fw-pack.py` has no file-level notice; its repository root license is
Apache-2.0. This decision applies only to `golden.json` and is not blanket
clearance for other source material or generated data.

The host state and inputs in `capture.c` are independently authored. The
`EEPROM_Config_t` mock in `settings_capture_support.h` is not independent: it is
a minimized, modified subset of the declaration in pinned `settings.h`. It
retains the source's Copyright 2023 Dual Tachyon and Apache-2.0 notice, marks
the subset as modified, and is accompanied by a copy of the Apache-2.0 license
at `tools/reference-vectors/LICENSE-APACHE-2.0`. The capture harness includes
pinned `misc.h`/`frequencies.h` definitions and extracts the `TxLockModes_t`
enum from pinned `settings.h` into its temporary build directory; those
temporary upstream definitions are not included in this repository. The mock
decision covers only the identified test-only subset, not any other upstream
declarations.
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

The host-only consumer at
[`tools/reference-vectors/runner.py`](../tools/reference-vectors/runner.py)
loads the checked-in fixture directly; it does not require the C checkout or
regenerate the baseline. It validates the pinned commit, case fields, input and
expected-value types, unique vector IDs, EEPROM write lengths, and checksum
metadata before running registered domain evaluators. A mismatch reports the
vector ID with expected and actual values. Domains without an evaluator are
reported as unported and are never counted as covered.

[`tools/reference-vectors/compare_rust.py`](../tools/reference-vectors/compare_rust.py)
passes compatible fixture inputs to the host-only
[`examples/reference_vectors.rs`](../examples/reference_vectors.rs) bridge.
The bridge calls the production library's settings encoder, channel-validity
predicate, frequency checks and rounding, and CTCSS/DCS functions; the typed
runner compares those results against the fixture. The command builds/runs
Cargo with `--offline`, does not invoke the optional C reference, reports
every executed and unexecuted case, and exits nonzero while any cases remain
unported:

```sh
python3 tools/reference-vectors/compare_rust.py
```

Host coverage is 38 of 39 vectors: 12 channel, 15 frequency, 5 DCS, 4 CTCSS,
and 2 settings profiles. The single
`firmware_container_checksum/byte_ramp_12k` vector remains unsupported because
the Rust library has no updater-container packing or checksum API. Its payload,
metadata, packed-image hash, and appended CRC concern the updater container,
not EEPROM settings or a settings checksum. Neither unsupported domain is
counted as parity coverage. The opt-in pinned C comparison remains available
separately through `tools/reference-vectors/compare.py`, with a clean pinned C
checkout supplied explicitly:

```sh
UV_K5_C_REFERENCE=/path/to/clean/uv-k5-firmware-custom \
  python3 tools/reference-vectors/compare.py
```

The host test suite validates this reader and runner against the approved
fixture, including malformed input, a seeded mismatch, and a Rust-bridge
regression that changes an expected result and requires a mismatch:

```sh
python3 -m unittest discover -s tests -v
```

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

The host bridge and suites were run with the following command and evidence:

```sh
rustc --version
cargo --version
python3 --version
python3 tools/reference-vectors/compare_rust.py
cargo test --offline --locked --all-targets
python3 -m unittest discover -s tests -v
```

The host reported `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, `cargo 1.99.1
(5f94df478 2026-08-27)`, and `Python 3.12.3`. The bridge reported `38/39
matched (38 executed)`, with no mismatches; only the firmware-container
checksum case remains unsupported, so the command returns status 1 because the
fixture is not fully covered. The Rust suite passed 58 tests; the Python suite
passed 46 tests with two existing ARM-binutils-dependent tests skipped. The
bridge used Cargo's offline mode. This is host-library evidence only, not a
device or firmware-image claim.

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

The repository's standard Rust CI is defined in
[`.github/workflows/rust-quality.yml`](../.github/workflows/rust-quality.yml):
it runs formatting, Clippy, host tests, and a Cortex-M0 release build with ELF
and map artifacts. It does not run these reference vectors. The separate
default C build evidence is recorded in [issue #11's evidence
comment](https://github.com/m0hdy/uv-k5-rust-firmware/issues/11#issuecomment-6982758020);
it is not a CI run of these vectors. In this sandbox, individual files were
read from the pinned revision and their Git object IDs verified. Captures with
GCC and Clang both matched the fixture. A full clean upstream checkout was
unavailable, so the capture's clean-checkout gate and the unittest command
above were not run here. The harness requires that gate for future
regeneration; this limitation is not a pass.

No firmware image was built or flashed for this capture. Physical-device
results are inapplicable because the work only records deterministic outputs
from source functions and uses a host EEPROM mock; device/recovery validation
remains tracked separately in [issue #10](https://github.com/m0hdy/uv-k5-rust-firmware/issues/10).
