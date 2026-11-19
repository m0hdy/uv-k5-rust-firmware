# Rust updater-container checksum reuse

The host-only Rust checksum evaluator uses the updater-container format
implemented by the pinned
[`fw-pack.py`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/fw-pack.py)
at commit `7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`. The 128-byte XOR
obfuscation table in that file was checked against the existing
`scripts/firmware_container.py` table and the pinned upstream source before
being reused in Rust. The source repository is Apache-2.0; the table's
provenance is retained here and alongside the Rust constant. `fw-pack.py` has
no file-level notice; the pinned repository's root license is Apache-2.0, with
the license text included at
[`tools/reference-vectors/LICENSE-APACHE-2.0`](../tools/reference-vectors/LICENSE-APACHE-2.0).

The Rust evaluator implements the 16-byte author/version insertion at offset
`0x2000`, repeating XOR obfuscation, and CRC-XMODEM serialized low byte first.
It is a host parity implementation only. The firmware-container bytes are
generated in Rust by the reference bridge; Python computes digests from those
emitted bytes. The canonical vector remains in `golden.json` unchanged.
