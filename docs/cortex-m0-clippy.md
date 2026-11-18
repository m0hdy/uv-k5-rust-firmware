# Cortex-M0 Clippy

Host Clippy does not compile code behind `#[cfg(target_arch = "arm")]`. The
`Cortex-M0 Clippy` workflow runs Clippy with the release profile for
`thumbv6m-none-eabi` so target-only Rust code is checked with warnings denied.
It uses the toolchain and target pinned in `rust-toolchain.toml` and does not
check out or require the C reference.

Run the same check locally from the repository root with the pinned Rust
toolchain and target installed:

```sh
cargo clippy --locked --target thumbv6m-none-eabi --release -- -D warnings
```

The existing `Rust quality` workflow retains the host checks and Cortex-M0
release build; this separate check adds target-specific lint coverage without
changing those checks.
