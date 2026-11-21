# Licensing and provenance

## Distribution scope

The original project work, including independently authored Rust modules,
tests, documentation, build scripts, and host tooling, is licensed under the
ISC License. The standard text and copyright notice are in
[`LICENSE`](../LICENSE).

The repository is not ISC-only. Confirmed adaptations and copied data from the
pinned `egzumer/uv-k5-firmware-custom` reference remain under Apache-2.0.
Redistributions that contain those items must include
[`LICENSE-APACHE-2.0`](../LICENSE-APACHE-2.0) and
[`NOTICE`](../NOTICE). The detailed item inventory and reuse decisions are in
[`reference-provenance.md`](reference-provenance.md).

## Inventoried material

| Material | Shipping scope | Rights and handling |
| --- | --- | --- |
| Original Rust source, tests, build configuration, and independently authored models | Source and any Rust-linked object or binary | ISC; copyright Damien Hardy (`@digimangos`) |
| `gFontSmall` Rust conversion | `src/startup_version_display.rs` | Copyright 2023 Dual Tachyon; Apache-2.0; converted and attributed in place |
| Tuning, CTCSS/DCS, and squelch adaptations | `src/tuning_signalling.rs` | Copyright 2023 Dual Tachyon; Apache-2.0; source-derived portions retain notices and modification attribution |
| Reference golden fixture | `tests/reference-vectors/golden.json` | Captured test data from pinned upstream outputs; Apache-2.0 provenance applies to the selected source-derived results, with no copied implementation |
| Firmware-container obfuscation table | `scripts/firmware_container.py` and linked Rust container logic | Reproduced 128-byte protocol table under Apache-2.0; attribution and source revision are retained in provenance records |
| Settings capture support | `tools/reference-vectors/settings_capture_support.h` | Minimized, modified declaration derived from pinned `settings.h`; Apache-2.0, with notice and license copy beside it |
| Optional C reference and mixed fallback | C reference is external and not shipped; mixed-fallback fixtures are repository source | C reference is governed by its pinned Apache-2.0 terms. The repository's C and Rust fixture implementations are independently authored and governed by their repository provenance decisions |
| Reference captures and generated copies | Host-only tests, temporary build outputs, and checked-in golden data | Exact source, generator, revision, and reuse scope are recorded in `reference-vector-provenance.md`; temporary upstream definitions are not retained |
| Independently authored documentation and host scripts | Repository source and source archives | ISC |
| Bitmap and unapproved upstream assets | None | No reuse approved; not to be shipped |

## Release-bundle requirement

A firmware release or source archive containing a confirmed Apache-2.0 item
must ship `LICENSE`, `NOTICE`, and `LICENSE-APACHE-2.0` alongside the exact
artifact or source archive. Release packaging must copy these files
unchanged and must not describe the complete distribution as ISC-only.

The repository's current build is not a published firmware package. The
standalone Cortex-M0 bootstrap ELF is explicitly not flashable, and no
release-publishing workflow is added here. A draft bundle must therefore use
the exact artifact chosen for distribution and verify its SHA-256 separately
from the license files; this change does not publish or alter that artifact.
