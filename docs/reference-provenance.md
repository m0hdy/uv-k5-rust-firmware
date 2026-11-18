# Reference source provenance

## Pinned C reference

The optional C reference is [egzumer/uv-k5-firmware-custom](https://github.com/egzumer/uv-k5-firmware-custom), pinned to commit [`7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`](https://github.com/egzumer/uv-k5-firmware-custom/commit/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3). The reference repository is used only to compare behavior or support an explicitly requested mixed build; it is not vendored, modified, or required to build the Rust firmware.

To select an existing local checkout for parity or mixed-build tooling, set `UV_K5_C_REFERENCE` to its path. Before using it, require its checked-out `HEAD` to be the pinned commit:

```sh
expected=7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3
: "${UV_K5_C_REFERENCE:?set this only when using the optional C reference}"
actual=$(git -C "$UV_K5_C_REFERENCE" rev-parse --verify HEAD)
test "$actual" = "$expected" || {
    printf 'C reference must be at %s (found %s)\n' "$expected" "$actual" >&2
    exit 1
}
```

If a local checkout is needed, it can be obtained on demand from the URL above and checked out at that commit. There is no automatic fetch, reference selector, or Rust build integration in the current repository. Normal Rust builds must neither read `UV_K5_C_REFERENCE` nor depend on network access or the C source.

## Source and tool versions

The pinned C repository's `README.md` recommends `arm-none-eabi-gcc` 10.3.1; its Windows setup instructions identify Arm GNU Toolchain 10 2021.10. The upstream Makefile uses `arm-none-eabi-gcc`, `arm-none-eabi-objcopy`, and `arm-none-eabi-size`. Packaging uses Python and `crcmod`; the Windows instructions specify Python 3.8 but do not pin `crcmod`. The optional upstream Dockerfile uses `archlinux:latest` and installs tool packages without version pins. Clang is described as experimental and is not version-pinned. These are provenance for the C reference's build guidance, not dependencies of the Rust firmware.

The current Rust repository has no Cargo manifest, Rust toolchain file, build scripts, or test configuration, so it declares no Rust tool versions here.

## Reuse inventory and decisions

This inventory describes the current Rust repository and the pinned source locations that were inspected. No C code, tables, assets, generated data, or test vectors from the reference have been copied into this repository.

| Material | Source location and revision | License and notices observed | Current reuse decision |
| --- | --- | --- | --- |
| Rust implementation code | No Rust source files are present in the current repository. | No upstream source or notice applies. | None present; no upstream code reuse approved. |
| Tables and constants | Example source: [`frequencies.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/frequencies.c) at the pinned revision. Its file header says Copyright 2023 Dual Tachyon and includes the Apache-2.0 notice. | The pinned upstream root [`LICENSE`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/LICENSE) is Apache License 2.0. | Reference example only; no table or constant from it is imported. |
| Other C code | Example: [`am_fix.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/am_fix.c) at the pinned revision. Its file header says Copyright 2023 OneOfEleven, links DualTachyon, and includes the Apache-2.0 notice. | Apache-2.0 at the repository root, plus the file-level attribution and license notice. | Reference example only; no code from it is imported. |
| Bitmap assets | Example: [`bitmaps.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/bitmaps.c) at the pinned revision. The inspected file has no file-level copyright or license header; the upstream root declares Apache-2.0. | Confirm applicable asset attribution and notice details before any reuse; the root license alone does not resolve missing asset-specific provenance. | Not imported; reuse is not approved pending that review. |
| Generated data and test vectors | None present in the current Rust repository; no reference-derived generated data or vectors are identified as imported. | No upstream notices apply to material not imported. | None present; no reuse approved. |
| Project description | The one-line description in this repository's [`README.md`](../README.md). | No upstream attribution or copied material has been identified in that description. | Project description only; no reference material identified. |

The pinned upstream README describes its codebase as a merge of OneOfEleven custom firmware, fagci spectrum analyzer work, and DualTachyon firmware, with additional changes and contributor credits. This is useful source history, not blanket clearance for every file or asset. No repository-root `NOTICE` file appears in the pinned tree listing.

## Approval and notice process

Before adding any material based on the C reference, record the exact upstream file or data location and pinned commit, identify its author and applicable license, inspect its file-level notices and any referenced third-party terms, and document whether the proposed use is a copy, adaptation, or independent implementation. If authorship, license scope, or required attribution is unresolved, do not copy or ship the material; leave it out until reviewed and approved.

For material confirmed to be under the upstream Apache-2.0 license, follow its redistribution terms: include a copy of the license, retain applicable copyright, patent, trademark, and attribution notices, mark modified files as changed, and preserve applicable notices from any distributed `NOTICE` file. A file without its own notice still requires provenance and rights review before reuse. Record each approved item and its required notices in the inventory above before committing it.

## Validation evidence

This change updates documentation only; no firmware was built, so there is no firmware artifact hash or per-device result to report. That is not a firmware validation pass. Record applicable CI runs, reports, artifact hashes, and per-device results on the issue or pull request when firmware validation is performed; explain any inapplicable check, and do not treat a skipped required check as a pass.
