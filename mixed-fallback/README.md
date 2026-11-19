# Opt-in mixed-fallback image

This software-only witness compares an independently authored C fallback and
Rust implementation of the narrow predicate “channel number is at most 199.”
That bound is supported by the public pinned reference's `RADIO_CheckValidChannel`
and `MR_CHANNEL_LAST = 199u` definitions; the provenance decision is recorded in
[`docs/reference-provenance.md`](../docs/reference-provenance.md). The C file
contains no copied upstream implementation.

The default Rust firmware and release pipeline are unchanged. Only the opt-in
features compile the C object, and the build script requires exactly one
selection:

```sh
cargo build --locked --target thumbv6m-none-eabi --release \
  --bin mixed-fallback --features mixed-fallback,mixed-fallback-c
cargo build --locked --target thumbv6m-none-eabi --release \
  --bin mixed-fallback --features mixed-fallback,mixed-fallback-rust
```

Run the host comparison and both full image validations, and generate the
toolchain/source/artifact evidence:

```sh
UVK5_RUN_MIXED_FALLBACK=1 \
  python3 -m unittest discover -s tests -p 'test_mixed_fallback.py' -v
python3 mixed-fallback/run.py --report mixed-fallback/evidence.md
```

The opt-in image includes the existing `src/bin/startup.S` and uses
`linker/firmware.ld`. The runner proves that the selected implementation is
called from `firmware_main`, the other implementation is absent from the
linked symbol table, and each ELF/raw pair passes the startup and memory-budget
checks with the existing 2 KiB stack reservation. Both Cargo builds use a
runner-owned target directory under the selected output directory, regardless
of an ambient `CARGO_TARGET_DIR`. No device execution or release artifact is
involved.

The path-scoped `Mixed-fallback evidence` workflow runs this same runner for
relevant pull requests and can also be started with **Actions → Mixed-fallback
evidence → Run workflow**. It checks out the exact pull-request head, publishes
the runner's report in the job summary, and retains only that diagnostic report
as a five-day artifact. The workflow downloads the artifact and compares it
with the generated report; a missing or changed report fails the job. The
default Rust firmware and its CI artifacts are unchanged.
