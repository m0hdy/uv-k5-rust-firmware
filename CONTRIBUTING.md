# Contributing

This repository is a development project. It has no supported, flashable Rust
firmware release; CI artifacts are unvalidated development outputs. Do not
flash them or describe them as release-ready.

## Start with an issue

Open or find an issue before starting substantial work. Describe the problem,
proposed change, and acceptance criteria, and link related work in the pull
request. Keep changes focused and avoid assuming undocumented radio behavior or
hardware compatibility.

## Checks

For host-side Rust changes, run the checks relevant to the change:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
python3 -m unittest discover -s tests -v
```

For startup, linker, or target changes, also build the Cortex-M0 target and
check the resulting ELF and map:

```sh
cargo rustc --locked --target thumbv6m-none-eabi --release \
  --bin uv-k5-rust-firmware -- \
  -C link-arg=-Map=target/thumbv6m-none-eabi/release/uv-k5-rust-firmware.map
python3 scripts/check_startup_layout.py \
  --elf target/thumbv6m-none-eabi/release/uv-k5-rust-firmware \
  --map target/thumbv6m-none-eabi/release/uv-k5-rust-firmware.map
```

For an image change, retain evidence for the exact source commit and tool
versions, ELF, linker map, raw payload, packed container, check reports, and
individual file hashes. Run the documented image gate against that exact ELF
and raw pair; packing and offline checks do not establish device compatibility
or safe flashing. See the [target contract](docs/target-contract.md) and
[offline image evidence](docs/offline-image-evidence.md).

## Device testing

Device tests are not implied by a successful build or offline check. Follow
the [recovery runbook](docs/recovery-runbook.md) before any device testing; it
defines the no-flash preflight, required evidence, and stop conditions. Never
flash an unqualified development image.

## Pull requests

Link the tracking issue and describe the change, verification performed, and
any checks not run. State explicitly whether hardware testing was performed;
if so, record the radio model and revision and summarize the result. Redact
serial numbers, EEPROM/calibration contents, and other private device data
from logs and attachments.
