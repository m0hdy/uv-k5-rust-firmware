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

The checked-in C/Rust golden fixture captures settings writes only; it does not
capture `SETTINGS_InitEEPROM` read/default behavior. Rust now has pure startup
decoders for the core, display, channel-selection, and standard extended blocks
(`0x0E90` through `0x0F40`), with their source mapping documented in
[`settings-extended-boot-provenance.md`](settings-extended-boot-provenance.md).
The full C initialization path is still not executed by the host parity
fixture. Standard block acquisition is available as a portable transport
composition, but target-entry wiring and optional FM data, DTMF calling
strings, AES key handling, channel attributes, and calibration loading remain
uncovered.
No settings checksum routine was identified in `SETTINGS_SaveSettings`; the
firmware-container CRC fixture does not test settings validation or a settings
checksum.

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

The host Rust evaluator now executes and matches all 39 vectors: 12 channel,
15 frequency, 5 DCS, 4 CTCSS, 2 settings profiles, and 1 firmware-container
checksum. It passes the checksum vector through the Rust container API and
compares the packed output metadata with the fixture. Run it from the
repository root:

```sh
python3 tools/reference-vectors/compare_rust.py
```

The report is `Vector results: 39/39 matched (39 executed).` It compares each
Rust result to the fixture and reports the vector ID plus expected and actual
values for mismatches. Missing evaluators are reported as unported, not counted
as coverage, and an incomplete or mismatched report exits nonzero.

The separate C comparator runs the capture against the pinned source and
compares each captured vector's identity, inputs, and result with the canonical
fixture:

```sh
UV_K5_C_REFERENCE=/path/to/clean/uv-k5-firmware-custom \
  python3 tools/reference-vectors/compare.py
```

`UV_K5_C_REFERENCE` must point to a Git checkout whose `HEAD` is exactly
`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3` and whose tracked and untracked
working tree is clean. The comparator rejects a missing path setting, a
different revision, or any working-tree changes before compiling or running
the capture. It does not update the fixture. Input/result differences and
missing, unexpected, or duplicate cases are reported as mismatches, and the
command exits nonzero.

Fixture regeneration is explicit and separate from both reports. It requires
the same clean pinned checkout, host GNU C2x compiler (`cc` by default, or
`CC`), Python 3, and `crcmod==1.7` for the pinned packer:

```sh
python3 -m pip install crcmod==1.7
UV_K5_C_REFERENCE=/path/to/clean/uv-k5-firmware-custom \
  python3 tools/reference-vectors/capture.py --write
```

`--write` overwrites `tests/reference-vectors/golden.json`; review the change
before accepting it. To verify a capture without changing the fixture, use
`capture.py --check` with the same environment setting.

The narrow host tests for C comparison mismatch details and wrong-revision
rejection, and for Rust's 39-case match and expected/actual mismatch reporting,
are:

```sh
python3 -m unittest discover -s tests -p test_compare.py -v
python3 -m unittest discover -s tests -p test_reference_vectors.py -v
```

The default CI host suite runs these tests, including the Rust evaluator,
without invoking the pinned C comparison or inspecting `UV_K5_C_REFERENCE`.
A full checkout of the pinned C repository is needed for `compare.py` or
capture/regeneration. The fixture
SHA-256 is
`361172955485ba9d7b6cd3b26cafc0f73cc8ed9e56ee84d79c36b8da6049c748`.
Neither report validates EEPROM read/default handling: `SETTINGS_InitEEPROM`
also depends on UI, radio, and device helpers, so malformed EEPROM reads and
their defaults remain outside this fixture. The vectors exercise selected
host-callable functions and inputs; they do not establish full firmware
integration, hardware behavior, bootloader acceptance, image validity, or
device/recovery safety.

## Hosted canonical parity evidence

The path-scoped
[canonical parity workflow](../.github/workflows/canonical-parity-evidence.yml)
runs on pull requests that touch the canonical vector tooling, fixture,
Rust bridge or implementation, toolchain contract, or this provenance record.
It can also be started manually with `workflow_dispatch`. The workflow checks
out the exact pull-request head (or dispatch revision) separately from the
pinned C revision, installs the Rust toolchain from that Rust checkout's
`rust-toolchain.toml`, and installs `crcmod==1.7` in a runner-owned virtual
environment. Python bytecode generation is disabled. A manual
`fail-c-comparator` dispatch probe intentionally makes the C comparison fail;
the report must show that failure, the Rust comparison must still complete,
and the workflow job must fail.

The job verifies the C checkout revision and clean working tree before and
after `compare.py`, verifies the fixture retains the documented SHA-256, and
runs both existing comparators without invoking fixture regeneration. The C
compiler must accept GNU C2x. Both comparisons must pass all 39 canonical
cases; a nonzero comparator status is recorded, the other comparator still
runs, and the job then fails. A separate report check requires both complete
summaries, successful statuses, the exact Rust source revision, and the
unchanged fixture hash.

The workflow records source revisions, Python/Rust/C compiler and `crcmod`
versions, the fixture SHA-256, clean-checkout checks, and full comparator
output in one diagnostic report. That report is uploaded for three days only.
This evidence is limited to the canonical corpus and does not establish full
firmware parity or hardware results.

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
and map artifacts. The host tests run the Rust reference-vector evaluator,
but not the opt-in pinned C comparison. The separate
default C build evidence is recorded in [issue #11's evidence
comment](https://github.com/m0hdy/uv-k5-rust-firmware/issues/11#issuecomment-6982758020);
it is not a CI run of these vectors. Captures with GCC and Clang previously
matched the fixture. The documented C comparison and read-only capture check
were also verified against a clean pinned checkout, and the Rust report
independently matched all 39 vectors. Both focused test commands passed.

No firmware image was built or flashed for this capture. Physical-device
results are inapplicable because the work only records deterministic outputs
from source functions and uses a host EEPROM mock; device/recovery validation
remains tracked separately in [issue #10](https://github.com/m0hdy/uv-k5-rust-firmware/issues/10).
