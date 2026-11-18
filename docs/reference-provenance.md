# Reference source provenance

## Pinned C reference

The optional C reference is [egzumer/uv-k5-firmware-custom](https://github.com/egzumer/uv-k5-firmware-custom), pinned to commit [`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/commit/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3). The reference repository is used only to compare behavior or support an explicitly requested mixed build; it is not vendored, modified, or required to build the Rust firmware.

To select an existing local checkout for parity or mixed-build tooling, set `UV_K5_C_REFERENCE` to its path. Parity must use a clean checkout at the pinned commit: checking `HEAD` alone does not detect changed source inputs. Require both the expected commit and an empty working tree (including no untracked files):

```sh
set -eu
expected=7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3
: "${UV_K5_C_REFERENCE:?set this only when using the optional C reference}"
actual=$(git -C "$UV_K5_C_REFERENCE" rev-parse --verify HEAD)
test "$actual" = "$expected" || {
    printf 'C reference must be at %s (found %s)\n' "$expected" "$actual" >&2
    exit 1
}
status=$(git -C "$UV_K5_C_REFERENCE" status --porcelain --untracked-files=all)
test -z "$status" || {
    printf 'C reference must have a clean working tree:\n%s\n' "$status" >&2
    exit 1
}
```

If a local checkout is needed, it can be obtained on demand from the URL above and checked out at that commit. `UV_K5_C_REFERENCE` is the optional local reference selection; normal Rust builds must not require it, network access, or the C source.

## Source and tool versions

The pinned C repository's `README.md` recommends `arm-none-eabi-gcc` 10.3.1; its Windows setup instructions identify Arm GNU Toolchain 10 2021.10. The upstream Makefile uses `arm-none-eabi-gcc`, `arm-none-eabi-objcopy`, and `arm-none-eabi-size`. Packaging uses Python and `crcmod`; the Windows instructions specify Python 3.8 but do not pin `crcmod`. The optional upstream Dockerfile uses `archlinux:latest` and installs tool packages without version pins. Clang is described as experimental and is not version-pinned. These are provenance for the C reference's build guidance, not dependencies of the Rust firmware.

## Reuse inventory and decisions

This inventory records the pinned reference examples inspected for provenance. It does not approve any material for import. Before any reference-derived code, table, asset, generated data, or test vector is added, record its exact source location and revision, applicable license and notices, and a reuse decision here. Independently written material should be identified as such; uncertain provenance or obligations mean the material is not approved for reuse.

| Material | Source location and revision | License and notices observed | Current reuse decision |
| --- | --- | --- | --- |
| Rust implementation code | The Rust implementation in this repository; list each source path and any source revision if derived from upstream material. | For independent work, record its authorship; for derived work, inspect file-level notices and applicable license terms. | No imported material is approved by this document. Record independently written code as such; otherwise hold pending review. |
| Tables and constants | Example source: [`frequencies.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/frequencies.c) at the pinned revision. Its file header says Copyright 2023 Dual Tachyon and includes the Apache-2.0 notice. | The pinned upstream root [`LICENSE`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/LICENSE) is Apache License 2.0. | Reference example only; no reuse is approved. |
| Other C code | Example: [`am_fix.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/am_fix.c) at the pinned revision. Its file header says Copyright 2023 OneOfEleven, links DualTachyon, and includes the Apache-2.0 notice. | Apache-2.0 at the repository root, plus the file-level attribution and license notice. | Reference example only; no reuse is approved. |
| Bitmap assets | Example: [`bitmaps.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bitmaps.c) at the pinned revision. The inspected file has no file-level copyright or license header; the upstream root declares Apache-2.0. | Confirm applicable asset attribution and notice details before any reuse; the root license alone does not resolve missing asset-specific provenance. | No reuse is approved pending that review. |
| Generated data and test vectors | For any proposed item, record its generating source, exact input/source revision, and repository path. | Determine whether the data or vectors carry upstream rights or required notices before use. | No reuse is approved by this document. |

The pinned upstream README describes its codebase as a merge of OneOfEleven custom firmware, fagci spectrum analyzer work, and DualTachyon firmware, with additional changes and contributor credits. This is useful source history, not blanket clearance for every file or asset. No repository-root `NOTICE` file appears in the pinned tree listing.

## Approval and notice process

Before adding any material based on the C reference, record the exact upstream file or data location and pinned commit, identify its author and applicable license, inspect its file-level notices and any referenced third-party terms, and document whether the proposed use is a copy, adaptation, or independent implementation. If authorship, license scope, or required attribution is unresolved, do not copy or ship the material; leave it out until reviewed and approved.

For material confirmed to be under the upstream Apache-2.0 license, follow its redistribution terms: include a copy of the license, retain applicable copyright, patent, trademark, and attribution notices, mark modified files as changed, and preserve applicable notices from any distributed `NOTICE` file. A file without its own notice still requires provenance and rights review before reuse. Record each approved item and its required notices in the inventory above before committing it.

## Validation evidence

This document change does not report firmware validation. Record applicable CI runs, reports, artifact hashes, and per-device results on the issue or pull request when firmware validation is performed; explain any inapplicable check, and do not treat a skipped required check as a pass.
