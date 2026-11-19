# Offline ELF and packed-image evidence

## Scope and source

This record covers the Cortex-M0 bootstrap built from main commit
[`8cc6cfc6563576fb51c53d49d1ee98b258c53793`](https://github.com/m0hdy/uv-k5-rust-firmware/commit/8cc6cfc6563576fb51c53d49d1ee98b258c53793).
It is a measurement of that exact build, not a projection of full-firmware size.
The files were rebuilt locally from that commit with the pinned Rust 1.99.1
toolchain, GNU ARM binutils 2.42, and Python 3.12.3. CI produced byte-identical
ELF, map, raw, and packed files for the same commit.

The matching hosted evidence is in [Rust quality run 44433163813](https://github.com/m0hdy/uv-k5-rust-firmware/actions/runs/44433163813)
(successful on `main`):

| Artifact | Artifact ID | SHA-256 of downloaded ZIP |
| --- | ---: | --- |
| `cortex-m0-target-report` | `14487676790` | `34dd6b51f8450a30f58e01fa884b75c946cf19290dec73f755d19a8746ca84c7` |
| `unvalidated-development-image` | `14487447695` | `473aee1a836335722b0ba6a9a4f800d9395ecba3269faa6bef4a8cca40afee13` |

The artifacts contain the ELF and map, plus the raw and packed outputs and
budget report. All four binary/map files match the local rebuild byte-for-byte.
The hosted artifact checksums above identify the downloaded ZIPs; the following
checksums identify the individual build outputs:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| ELF `uv-k5-rust-firmware` | 81,092 | `8544956cb9e90ee8069f5f2d087ecec039d30ea8ab975dd723b98aba0d5f5657` |
| Linker map | 3,858 | `3d2a8bc16e35b1651cbfa0981e4c259fed019748dbb21e41a15bb01416254ca5` |
| Raw objcopy payload | 300 | `febbcabeb65d724e573494cf64ef6dd6e0bd96a5e2ab3aaf9423030b87c25dbd` |
| Packed container | 318 | `0b9f64b6e28a243046cd42efd6ac22774fc4d13b7f8df71c66e1b3f2b86eb8df` |

## Startup and vectors

The startup-layout check passed against the ELF and map. The ELF is little-endian
ELF32/ARM and has entry `0x000000c1`, selecting Thumb state at `Reset_Handler`
(`0x000000c0`). The vector table occupies `[0x00000000, 0x000000c0)` (48
words/192 bytes); its initial stack pointer is `0x20003ff0`, and its reset-vector
word is `0x000000c1`, matching the ELF entry and Reset handler. The reserved
stack symbols span `0x20003800`–`0x20004000`.

## Measured layout and budget

The startup-layout and image-budget checks passed. The memory gate confirmed
that the raw payload spans the ELF's file-backed flash load bytes and matches
those bytes, including any objcopy padding. Current measured section sizes are:

| Allocated section | Size | Address / use |
| --- | ---: | --- |
| `.vector_table` | 192 bytes | Flash, `0x00000000` |
| `.text` | 92 bytes | Flash, `0x000000c0` |
| `.ARM.exidx` | 16 bytes | Flash, `0x0000011c` |
| `.data` | 0 bytes | Static RAM |
| `.bss` | 0 bytes | Static RAM |
| `.stack` | 2,048 bytes | Reserved RAM, `0x20003800`–`0x20004000` |

| Budget | Measured use | Capacity | Remaining |
| --- | ---: | ---: | ---: |
| Flash load | 300 bytes | 61,440 bytes | 61,140 bytes |
| Raw payload | 300 bytes | 61,440 bytes | 61,140 bytes |
| Static RAM | 0 bytes | 14,336 bytes below reserved stack | 14,336 bytes |
| Static RAM plus stack reservation | 2,048 bytes | 16,384 bytes | 14,336 bytes |

The packed-container length is 318 bytes and is reported separately; the raw
payload limit applies to the 300-byte raw file. These figures describe this
minimal bootstrap only. In particular, zero `.data`/`.bss` usage does not
estimate the eventual full firmware's static RAM or stack needs.

## Raw and packed relationship

The raw image was generated from this ELF with ARM `objcopy -O binary`. The
same raw file was packed using author `UV-K5` and version `dev`. Container
validation accepted that version field and CRC-XMODEM (`0x713d`, serialized as
`3d 71`). Unpacking the container recovered a 300-byte payload byte-for-byte
equal to the raw image. The hosted raw and packed files also match the local
rebuild exactly.

## Reproduction

From the repository root at the source commit above:

```sh
cargo rustc --locked --target thumbv6m-none-eabi --release \
  --bin uv-k5-rust-firmware -- \
  -D warnings \
  -C link-arg=-Map=target/thumbv6m-none-eabi/release/uv-k5-rust-firmware.map
out=target/thumbv6m-none-eabi/release
arm-none-eabi-objcopy -O binary \
  "$out/uv-k5-rust-firmware" "$out/uv-k5-rust-firmware.raw"
python3 scripts/firmware_container.py pack \
  "$out/uv-k5-rust-firmware.raw" "$out/uv-k5-rust-firmware.packed" \
  --author UV-K5 --version dev
python3 scripts/check_firmware_image.py \
  --elf "$out/uv-k5-rust-firmware" \
  --map "$out/uv-k5-rust-firmware.map" \
  --raw "$out/uv-k5-rust-firmware.raw" \
  --packed "$out/uv-k5-rust-firmware.packed" \
  --author UV-K5 --version dev
python3 scripts/firmware_container.py unpack \
  "$out/uv-k5-rust-firmware.packed" /tmp/recovered.raw \
  --author UV-K5 --version dev
cmp "$out/uv-k5-rust-firmware.raw" /tmp/recovered.raw
```

## Limits and device results

No device was tested. This offline evidence does not establish bootloader
acceptance, successful programming, or hardware behavior; the hosted files are
unvalidated development outputs, not flashable or release-ready firmware.
Per-device result: **not run** (no device test was part of this verification).
