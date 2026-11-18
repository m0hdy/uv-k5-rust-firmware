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
```

The crate has no external dependencies or allocator requirement.
The first three commands run on the host. The last builds the target-only
`no_std` entry stub; it does not produce a flashable firmware image.

## Reference firmware

The default commands above do not require or inspect a C firmware checkout or
any reference environment variable. For future parity tooling that explicitly
uses a reference checkout, select it with:

```sh
export UVK5_C_REFERENCE=/path/to/reference-firmware
```

No current build or test command consumes this variable.

## File ownership

This repository owns its Cargo and toolchain configuration, the host-testable
Rust library in `src/lib.rs`, and the target-only entry stub in
`src/bin/firmware.rs`. The C firmware remains the reference owner of shared
startup, linker, build/packaging, and ABI files. This bootstrap does not copy
or publish those files or define a flashable artifact.
