# uv-k5-rust-firmware

Standalone Rust migration target for the UV-K5(8). This bootstrap is not
flashable: it has no hardware startup/vector table, linker script, packaging,
or device-size checks.

## Requirements and checks

The repository pins Rust 1.99.1 and the `thumbv6m-none-eabi` target in
`rust-toolchain.toml`. With rustup installed, run these commands from the
repository root:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --target thumbv6m-none-eabi --release
python3 -m unittest discover -s tests -v
```

The crate has no external dependencies or allocator requirement.
The Cortex-M0 build produces a target-only `no_std` entry stub, not a
flashable firmware image. The memory-budget checker uses only the Python
standard library; the optional real-image test also uses GNU ARM binutils.

## Continuous integration

The `Rust quality` workflow runs on pushes and pull requests. Its stable required
checks are `Rust quality / Host checks` (formatting, Clippy, and host tests) and
`Rust quality / Cortex-M0 release build`. Each job installs the toolchain,
components, and target from `rust-toolchain.toml`; Cargo commands use the
tracked lockfile. Host checks also exercise the ELF/raw-payload memory-budget
checker with boundary and negative fixtures. The target job attaches only the
ELF and linker map as the `cortex-m0-target-report` artifact. Neither file is a
flashable image; the bootstrap ELF is intentionally not passed to the image
gate because it has no application memory layout.
The map/report upload runs even if a later gate step fails, provided the target
build completed successfully.

For any future flashable-image build, create the raw payload from the linked
ELF and run the gate before uploading, packaging for release, or otherwise
publishing any image:

```sh
arm-none-eabi-objcopy -O binary firmware.elf firmware.bin
python3 scripts/check_memory_budget.py \
  --elf firmware.elf \
  --raw firmware.bin \
  --stack-reservation 0x800 \
  --packed firmware.packed.bin
```

Omit `--packed` when no packed container is produced. The required 2 KiB stack
reservation is excluded from static RAM ranges and reserved at the top of RAM.
The checker rejects missing or malformed reservations and ordinary data
overlapping the stack. The raw payload limit is 61,440 bytes. A packed container's
length is reported separately and is not subject to that limit. The checker
prints flash-load, raw-payload, RAM, stack, and per-`PT_LOAD` usage and
remaining-capacity figures. The gate's implementation, accounting rules, and
fixture coverage are documented in [the target contract](docs/target-contract.md).

To confirm CI failure propagation without committing broken code, run the
workflow manually and select `host-test` or `target-build` for `failure_probe`.
That disposable run injects a failing test or target compile error and is
expected to fail. Locally, make the equivalent temporary injection in a
disposable worktree:

```sh
cat >> src/lib.rs <<'RS'
#[cfg(test)]
#[test]
fn intentional_failure_propagates() {
    assert!(false, "intentional CI failure probe");
}
RS
cargo test --locked --all-targets
```

For the target probe, append
`compile_error!("intentional Cortex-M0 build failure probe");` to
`src/bin/firmware.rs`, then run
`cargo build --locked --target thumbv6m-none-eabi --release`. Both commands
must return nonzero; do not commit either injection.

There is no C reference or parity job in the default workflow. The documented
`UV_K5_C_REFERENCE` environment variable is not consumed by current checks.
Any future mixed/parity lane must be separately opt-in, require an immutable
reference revision, and reject a missing, floating, or modified C checkout;
it must not fall back to an upstream branch. No image may be published until
the mandatory size gate from issue
[m0hdy/uv-k5-rust-firmware#26](https://github.com/m0hdy/uv-k5-rust-firmware/issues/26)
has passed for that exact ELF and raw payload in every flashable job.
Upload its linker map and budget report on an always-run path after the gate so
a gate failure still retains the diagnostics.

## Reference firmware

The default commands above do not require or inspect a C firmware checkout or
any reference environment variable. For future parity tooling that explicitly
uses a reference checkout, select it with:

```sh
export UV_K5_C_REFERENCE=/path/to/reference-firmware
```

No current build or test command consumes this variable.

## File ownership

This repository owns its Cargo and toolchain configuration, the host-testable
Rust library in `src/lib.rs`, and the target-only entry stub in
`src/bin/firmware.rs`. The C firmware remains the reference owner of shared
source and remains unchanged. The Rust migration's designated integration
owner is responsible for future Rust-side startup, linker, build/packaging, and
ABI integration. This bootstrap does not define or publish a flashable artifact.
