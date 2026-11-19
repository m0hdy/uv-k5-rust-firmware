# Portable Rust module size witness

## Result

Source revision `5d503cfc1ef70793bab0cf3c50718c2dddf09fb7` was built with
Rust 1.99.1 for `thumbv6m-none-eabi` in Cargo's default `dev` profile (no
`--release`, `opt-level=0`). The witness uses the repository linker layout and
its own reset/vector code. Calls to the settings and channel codecs,
tuning/signalling including squelch, scan state, and channel validity remain
referenced by the result written to a volatile sink. The witness map and ELF
contain the corresponding public symbols.

| Environment | Debug bootstrap flash | Witness flash | Witness static RAM | Reserved stack |
| --- | ---: | ---: | ---: | ---: |
| Original Linux measurement | 296 bytes | 34,832 bytes | 4 bytes | 2,048 bytes |
| Independent macOS measurement | 296 bytes | 34,992 bytes | 4 bytes | 2,048 bytes |

The Linux aggregate delta is +34,536 bytes flash and +4 bytes static RAM; the
reported macOS delta is +34,696 bytes flash and +4 bytes static RAM. The 160-byte
witness flash difference is an environment-scoped observation, not a universal
reproducibility guarantee. The independent macOS result was reported for the
same PR source and pinned Rust target; its complete tool versions and artifact
hashes were not recorded here.

The existing budget gate passed both images with
`--stack-reservation 0x800`. The witness leaves 26,608 bytes of the 61,440-byte
flash budget and 14,332 bytes of the 16,384-byte RAM-plus-stack budget in the
Linux measurement. The macOS witness also passed the gate. Its 4-byte static
RAM use is the result sink; the 2 KiB stack figure is a linker reservation,
not a measured stack high-water mark.

The witness has a 192-byte vector table, 34,624 bytes of `.text`, and 16 bytes
of `.ARM.exidx` in the Linux measurement. Its four-byte `.bss` is the volatile
result sink. The ELF's flash `PT_LOAD` covers 34,832 bytes, matching the raw
payload; zero-file-size RAM and stack segments do not add bytes to that payload.

## Reproduction

Run from the repository root with the pinned Rust toolchain, the installed
`thumbv6m-none-eabi` target, Python 3, and GNU Arm Embedded binutils:

```sh
set -eu
export CARGO_TARGET_DIR="$(mktemp -d "${TMPDIR:-/tmp}/portable-witness.XXXXXX")"

cargo rustc --locked --target thumbv6m-none-eabi \
  --bin uv-k5-rust-firmware -- \
  -C link-arg=-Map="$CARGO_TARGET_DIR/thumbv6m-none-eabi/debug/uv-k5-rust-firmware.map"

RUSTFLAGS="-C link-arg=-T$PWD/linker/firmware.ld -C link-arg=-Map=$CARGO_TARGET_DIR/thumbv6m-none-eabi/debug/portable-size-witness.map" \
  cargo rustc --manifest-path portable-witness/Cargo.toml --locked \
  --target thumbv6m-none-eabi --bin portable-size-witness

arm-none-eabi-objcopy -O binary \
  "$CARGO_TARGET_DIR/thumbv6m-none-eabi/debug/uv-k5-rust-firmware" \
  "$CARGO_TARGET_DIR/thumbv6m-none-eabi/debug/uv-k5-rust-firmware.raw"
arm-none-eabi-objcopy -O binary \
  "$CARGO_TARGET_DIR/thumbv6m-none-eabi/debug/portable-size-witness" \
  "$CARGO_TARGET_DIR/thumbv6m-none-eabi/debug/portable-size-witness.raw"

python3 scripts/check_startup_layout.py \
  --elf "$CARGO_TARGET_DIR/thumbv6m-none-eabi/debug/uv-k5-rust-firmware" \
  --map "$CARGO_TARGET_DIR/thumbv6m-none-eabi/debug/uv-k5-rust-firmware.map"
python3 scripts/check_startup_layout.py \
  --elf "$CARGO_TARGET_DIR/thumbv6m-none-eabi/debug/portable-size-witness" \
  --map "$CARGO_TARGET_DIR/thumbv6m-none-eabi/debug/portable-size-witness.map"
python3 scripts/check_memory_budget.py \
  --elf "$CARGO_TARGET_DIR/thumbv6m-none-eabi/debug/uv-k5-rust-firmware" \
  --raw "$CARGO_TARGET_DIR/thumbv6m-none-eabi/debug/uv-k5-rust-firmware.raw" \
  --stack-reservation 0x800
python3 scripts/check_memory_budget.py \
  --elf "$CARGO_TARGET_DIR/thumbv6m-none-eabi/debug/portable-size-witness" \
  --raw "$CARGO_TARGET_DIR/thumbv6m-none-eabi/debug/portable-size-witness.raw" \
  --stack-reservation 0x800
```

The witness command supplies an absolute path to the existing linker script
because Cargo resolves the root configuration's relative linker-script path
from the separate package directory. The `RUSTFLAGS` override replaces that
configuration for the witness invocation and adds the map output; it does not
change the linker script or any tracked build configuration.

The map and demangled ELF symbol table showed retained codec methods
(`Settings::encode`, `SettingsImage::decode`, and
`ChannelRecord::{encode,decode}`), `ScanState::apply`,
`channel_validity::is_valid_channel`, the tuning/signalling entry points used
by the witness, and `configure_squelch`. Inspect the current artifacts with:

```sh
arm-none-eabi-nm -C --defined-only \
  "$CARGO_TARGET_DIR/thumbv6m-none-eabi/debug/portable-size-witness"
```

The bootstrap and witness use the same Rust/Cargo versions, target, linker
script, default `dev` profile, and budget checker. The link uses section
garbage collection, but neither build enables LTO or whole-program
optimization. The reported difference is an aggregate linked-image delta for
these exact inputs, not an additive per-module cost. Optimization, inlining,
section retention, and dead stripping in a release firmware build can change
the result substantially.

Other inputs that can affect a reproduction include the host OS and
architecture, the Rust host toolchain's compiler and LLVM versions, the
effective linker and binutils versions, Cargo/Rust flags and configuration, and
the workspace and output paths. Absolute paths can appear in maps and ELF
debug metadata, so artifact hashes are specific to their build environment and
paths; the hashes below identify the original Linux artifacts, not expected
cross-platform hashes. Compare measured flash load and budget-gate results
within each environment rather than treating ELF or map hash equality as a
requirement. The reproduction commands use a new temporary target directory
per run and do not remove or overwrite an existing build directory.

## Host validation

The following checks passed from the repository root:

```sh
cargo fmt --all -- --check
cargo fmt --manifest-path portable-witness/Cargo.toml -- --check
cargo clippy --locked --all-targets -- -D warnings
CARGO_TARGET_DIR=target/portable-witness \
  cargo clippy --manifest-path portable-witness/Cargo.toml --locked \
  --all-targets -- -D warnings
cargo test --locked --all-targets
CARGO_TARGET_DIR=target/portable-witness \
  cargo test --manifest-path portable-witness/Cargo.toml --locked
ARM_BINUTILS_PREFIX=arm-none-eabi- python3 -m unittest discover -s tests -v
```

The Python suite ran 92 tests; 2 environment-conditional tests were skipped.
No firmware image was uploaded or run on a device.

## Fixture and behavior scope

The checked-in canonical reference fixture records ten 8-byte settings writes
at their EEPROM offsets for two selected profiles. It establishes byte parity
for those captured writes and profiles only. It does not establish all possible
EEPROM contents, EEPROM read/default/clamp behavior, write transaction
behavior, hardware EEPROM behavior, or a settings checksum. Separately,
`tests/channel_codec.rs` checks the 16-byte channel-record layout and codec
round trips; that is not broader EEPROM-device validation.

The canonical fixture's channel cases exercise the selected channel-validity
inputs. Its other vectors cover selected tuning/signalling outputs, but it has
no scan-state or squelch-threshold vectors. The Rust host tests exercise
additional codec and tuning boundaries, scan decisions, and the source-derived
squelch transformation; those Rust-only cases do not become independent C
parity evidence by appearing in this size witness. In particular, the
squelch tests do not establish EEPROM threshold tables or RF detection
behavior. The separate updater-container vector is not called by this
witness and is not evidence about EEPROM bytes or the measured firmware image.

No firmware was uploaded and no device was executed. This is a debug-profile
link measurement and not a release-size estimate, hardware validation, or
claim about radio behavior.

## Provenance

Original Linux build environment: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`,
LLVM 23.1.1, Cargo 1.99.1, GNU Arm binutils 2.42. The measured source
revision and SHA-256 hashes for that build are:

| Artifact | SHA-256 |
| --- | --- |
| Witness ELF | `5cc92f0114dbd834e87bc5da2579d7163afa17d694743320688b41296ddc81f3` |
| Witness linker map | `375af1489cc3b8fb297a2751227c3166dc59acdc4bbf3047c3f2b19e2837f8ac` |
| Witness raw payload | `eeb1bbf00d2f7da7e835c4c04e07a38ca7e17b725901f1c4527935aa9b619a25` |
| Bootstrap ELF | `4f2d50132ef39f528337fafed49c947423ee95ff74df803b4c75f857f7592c4f` |
| Bootstrap linker map | `c6c354bbb49b7cd8d2573234bd2bdd61a0e9c95bdfad88f4def2177e624c05ed` |
| Bootstrap raw payload | `984f9976247078c59a3db2f063d654a4a02c3565fd12ba7d5c2345d63f18d617` |
| `portable-witness/src/lib.rs` | `19ccf74489ff6104a077707047b7d5e9bf8ad3c7330d16f470a9f63cc4549a38` |
| `portable-witness/src/main.rs` | `656e80bc77996ffe346640416096ec9c6cabcac933375e473cb696cbfe5f68d1` |
| `portable-witness/src/startup.S` | `a0f7b2859cab48606c841958cfbd10b6e31d7179fdbbd8cf29f33190acb32987` |
| `portable-witness/Cargo.toml` | `460a1f86955c7bc0b3dabfe67b397aacdd93f6605901651a426b8b0e41ba8faa` |
| `portable-witness/Cargo.lock` | `5328312b9d43b5c2eca671a1a08f8548400b45089613535ebdec81441ddaecc9` |
| `portable-witness/tests/host.rs` | `ec857822c27cb2f922f3061981fe133ff7cb57e0cae131a7be316cab169998e4` |
| Root `Cargo.lock` | `e88838aa61f4d694a359c8e7d53bcb39aa2f5ba730905dd2b53d41e57995fe95` |
| `rust-toolchain.toml` | `fb09040ab142480182f468a63f6c7be9e64084523ecd7c93ad9b932a75800e89` |
| `linker/firmware.ld` | `db5d0c300aa57de5bd9a3d68c67b993097ab4ce5bd05e8fd33a6080b11174226` |
