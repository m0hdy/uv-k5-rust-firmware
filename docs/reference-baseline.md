# C reference baseline

## Status

The default C build has been reproduced twice. This is build evidence only:
the [bench and recovery task](https://github.com/m0hdy/uv-k5-rust-firmware/issues/12)
and [golden-output task](https://github.com/m0hdy/uv-k5-rust-firmware/issues/13)
remain open. No radio was flashed or tested during these builds.

## Source and configuration

| Item | Baseline |
| --- | --- |
| Upstream | <https://github.com/egzumer/uv-k5-firmware-custom> |
| Commit | [`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/commit/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3) |
| Configuration | `Makefile` defaults at that commit; override none of the `ENABLE_*` options |
| Build identity | `AUTHOR_STRING=EGZUMER`; `VERSION_STRING` resolves to the exact tag or, otherwise, the short Git commit ID |
| Supported option values | Each `ENABLE_*` option is a `0`/`1` Make variable. This baseline records only the default configuration below; other combinations are not baseline builds. |

Default options from the pinned `Makefile`:

| Group | `1` (enabled) | `0` (disabled) |
| --- | --- | --- |
| Stock features | `ENABLE_UART`, `ENABLE_FMRADIO`, `ENABLE_VOX`, `ENABLE_DTMF_CALLING`, `ENABLE_FLASHLIGHT` | `ENABLE_AIRCOPY`, `ENABLE_NOAA`, `ENABLE_VOICE`, `ENABLE_ALARM`, `ENABLE_TX1750`, `ENABLE_PWRON_PASSWORD` |
| Custom mods | `ENABLE_BIG_FREQ`, `ENABLE_SMALL_BOLD`, `ENABLE_CUSTOM_MENU_LAYOUT`, `ENABLE_KEEP_MEM_NAME`, `ENABLE_WIDE_RX`, `ENABLE_NO_CODE_SCAN_TIMEOUT`, `ENABLE_AM_FIX`, `ENABLE_SQUELCH_MORE_SENSITIVE`, `ENABLE_FASTER_CHANNEL_SCAN`, `ENABLE_RSSI_BAR`, `ENABLE_AUDIO_BAR`, `ENABLE_COPY_CHAN_TO_VFO`, `ENABLE_SPECTRUM`, `ENABLE_SCAN_RANGES` | `ENABLE_TX_WHEN_AM`, `ENABLE_F_CAL_MENU`, `ENABLE_CTCSS_TAIL_PHASE_SHIFT`, `ENABLE_BOOT_BEEPS`, `ENABLE_SHOW_CHARGE_LEVEL`, `ENABLE_REVERSE_BAT_SYMBOL`, `ENABLE_REDUCE_LOW_MID_TX_POWER`, `ENABLE_BYP_RAW_DEMODULATORS`, `ENABLE_BLMIN_TMP_OFF` |
| Debugging | none | `ENABLE_AM_FIX_SHOW_DATA`, `ENABLE_AGC_SHOW_DATA`, `ENABLE_UART_RW_BK_REGS` |
| Compiler/linker | `ENABLE_LTO` | `ENABLE_CLANG`, `ENABLE_SWD`, `ENABLE_OVERLAY` |

The Makefile also recognizes `ENABLE_BACKLIGHT_ON_RX`,
`ENABLE_SINGLE_VFO_CHAN`, and `ENABLE_BAND_SCOPE` as conditional compile
defines, but does not assign them defaults in this baseline. They are not
enabled for the recorded default configuration.

The linker script (`firmware.ld`) defines 60 KiB (61,440 bytes) of flash at
`0x00000000` and 16 KiB of RAM at `0x20000000`. It reserves 128 bytes for the
stack and no minimum heap. These are capacity limits, not measured application
usage.

## Tools and build procedure

The upstream README recommends `arm-none-eabi-gcc` 10.3.1. The upstream Docker
recipe installs from the moving `archlinux:latest` image, so it is not a
version-pinned reproducible environment. For comparable runs, record exact
versions of `arm-none-eabi-gcc`, `arm-none-eabi-binutils` (including
`objcopy`/`size`), Python, and `crcmod`, plus the host OS. The packer imports
`crcmod`; without it, the Makefile does not produce the packed image.

From a fresh, disposable checkout (the commands intentionally remove generated
and untracked files), run:

```sh
git clone https://github.com/egzumer/uv-k5-firmware-custom.git
cd uv-k5-firmware-custom
git checkout --detach 7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3
git submodule update --init --recursive
git status --short
git submodule status --recursive

# Record the exact tool versions before building.
arm-none-eabi-gcc --version
arm-none-eabi-objcopy --version
arm-none-eabi-size --version
python3 --version
python3 -m pip show crcmod

# The linker flags match the pinned Makefile defaults, with a map file added.
make clean
make 'LDFLAGS=-z noexecstack -mcpu=cortex-m0 -nostartfiles -Wl,-T,firmware.ld -Wl,--gc-sections --specs=nano.specs -Wl,-Map=firmware.map'
```

The build produces `firmware` (ELF), `firmware.bin` (raw image), and
`firmware.packed.bin` (packed image, if Python and `crcmod` are available);
`firmware.map` is enabled by the command above. `make` also prints the ELF
section-size report. Capture the files' lengths, flash and RAM section sizes,
and hashes:

```sh
wc -c firmware firmware.bin firmware.packed.bin firmware.map
arm-none-eabi-size firmware
arm-none-eabi-size -A firmware
sha256sum firmware firmware.bin firmware.packed.bin firmware.map
```

Repeat the same build from a clean checkout with the same tool versions and
configuration. Save each run's command log, tool versions, `git` and submodule
identities, size reports, and SHA-256 output. Compare the hashes between runs;
if they differ, investigate build inputs before treating either output as a
golden artifact. Retain the approved known-good packed image and its hash in an
authorized artifact store; do not attach firmware binaries to this repository
or issue.

### Results

Both clean builds passed on 2026-11-18 using Arm GNU Embedded 10.3-2021.10
(GCC 10.3.1, GNU objcopy 2.36.1.20210621), GNU Make 3.81, Python 3.10.4,
and crcmod 1.7 on macOS arm64 with Rosetta. The ELF, raw binary, and packed
image were byte-identical across the two runs.

| Evidence | Both runs |
| --- | --- |
| ELF SHA-256 | `f0cc538befe1f2f649b09c8943d9091d8755c0a76e649679c77228b16a3276c9` |
| Raw binary SHA-256 | `c3fb6bfde02154129d880b21f304872182debbca065ba6c3dd44d3bb5a25b36b` |
| Packed image SHA-256 | `bf24474da3607610c0f213827a01daa54b478c58beac950f7cd3238dc57592ae` |
| Raw payload / 61,440-byte flash slot | 60,800 bytes (640 remaining) |
| Packed container | 60,818 bytes |
| ELF `.text`, `.data`, `.bss` plus linker stack reservation (`size`) | 60,756 / 44 / 3,172 bytes |
| RAM `PT_LOAD` memory size, including 128-byte linker stack reservation | 3,216 / 16,384 bytes |

Flash occupancy includes the 44-byte `.data` load image. Linker-reserved RAM
is not a measurement of runtime stack margin. The map files differ only in
GCC's six-character LTO temporary object ID. Replacing that ID with
`<lto-id>` gives identical maps in this environment, SHA-256
`e5ff740068a6e6aaca75ff0cb8f0ed7517b4d69a1fd81b0614bcc9d63daa002d`.
See the [build evidence](https://github.com/m0hdy/uv-k5-rust-firmware/issues/11)
for the run record. A matching packed image is not a verified recovery image
until it has passed the device checks below.

## Bench and restoration record

Use the same radio, accessories, settings, test instruments, and firmware
identity for each comparison. Before testing, record the radio model/revision
and serial or internal test ID, firmware commit and packed-image hash, test
date/operator, power source, instrument IDs and calibration status, and a
backup of settings/EEPROM where supported. For each check below, record the
specific input values, observed outcome, pass/fail/not-run status, and evidence
reference. “Not run” is not a pass.

| Check | Inputs and procedure to record | Expected outcome to verify | Actual result / evidence |
| --- | --- | --- | --- |
| Cold boot | Fully power off for a recorded interval; note saved channel/frequency, volume, and settings before power-off. | Boots without reset loop; display and saved configuration are available. | Not run |
| Warm reset | Use the documented reset/restart path; record state before and after. | Restarts without reset loop and preserves the expected saved settings. | Not run |
| Display and keys | Exercise each key and display state used by the matrix; note backlight and menu state. | Display is legible and each key produces its documented action. | Not run |
| Receive and audio | Use a known, shielded or suitably attenuated signal; record frequency, mode, level, squelch, and audio path. | Correct signal indication and intelligible audio without unintended output. | Not run |
| Transmit | Only by an authorized operator in a controlled, legally permitted bench setup; use a dummy load and appropriate rated attenuation/instrumentation, not an antenna. Record frequency, power, duration, and measured output. | Transmit only on the authorized test setup; measured output and modulation meet the bench acceptance limits. | Not run; no transmit test claimed |
| Scan | Record programmed range/channels, step, mode, squelch, and active scan list. | Scan visits the configured entries and stops/resumes as expected. | Not run |
| Channel/frequency changes | Record starting value, key/menu sequence, selected channel or entered frequency, and readback. | Selected value is correct and remains correct after returning to the display. | Not run |
| EEPROM/settings | Record initial backup, changed fields, save path, and power cycle. | Intended values persist; unrelated settings remain unchanged. | Not run |
| UART programming | Record programmer/software version, cable, image/configuration, operation, and readback. | Programming completes and the radio reports the expected identity/settings. | Not run |
| Battery indication | Record measured supply voltage, charging state, displayed level, and applicable calibration/settings. | Indication follows the reference acceptance range for the measured condition. | Not run |

### Recovery checklist

The mandatory preflight and rollback sequence is in the
[recovery runbook](recovery-runbook.md).
No known-good packed image or successful restoration has been recorded yet.
Complete and retain this evidence on a designated recoverable test radio before
claiming a rollback path:

1. Confirm the exact radio model and hardware revision are supported by the
   image; record the radio test ID and backup settings/EEPROM.
2. Verify the retained packed image's SHA-256 against its build record and
   confirm the approved programming method and recovery procedure are available.
3. Use a charged, stable power source and the controlled bench setup. Record
   programmer/software versions, cable, image hash, start/end time, and all
   status/error messages.
4. Re-flash the recovery image approved for the baseline attempt, reboot, and
   verify boot, display, keys, receive, channel/frequency readback, settings,
   and UART readback as applicable.
5. Record the observed results, device and build identities, and evidence link.
   Do not report restoration success until these checks have actually passed.

## Golden-output provenance

No derived fixtures or expected-output files are included. Before publishing
any fixture, link it to a provenance review and record the source and exact
revision, applicable license/permission, generation method and tool versions,
input case, expected output, reviewer, and fixture hash. Follow the
[per-item provenance process](reference-provenance.md) before committing
anything derived from the reference.

The planned case set covers tuning and channel validation (valid values,
boundaries, and invalid values), DCS/CTCSS encoding and detection, and settings
encoding/checksums (valid, boundary, and invalid inputs). For each case, capture
input, expected output, actual output, source/generator identity, and the C
build identity. No expected values or actual outputs have been asserted yet.
