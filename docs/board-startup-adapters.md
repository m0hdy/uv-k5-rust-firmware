# Board startup adapter witness

`src/board_startup_adapters.rs` dispatches the existing sequencer into the
clock-gate, SysTick, PortCon, and GPIO target adapters. `ENABLE_SWD` is forwarded
to PortCon and `ENABLE_FMRADIO` to GPIO; the remaining board calls stay typed
placeholders. A failed SysTick setup stops further adapter calls before
PortCon or GPIO can run.

The host test checks the composed call order, feature forwarding, and stop
behavior. CI separately links `startup-witness/` for Cortex-M0 with a dedicated
vector table and a returning SysTick handler. Its linker places code at
`0x10000000` and RAM at `0x20010000`, outside the DP32G030 memory map, and
enforces 16 KiB code/load and 4 KiB RAM limits. The inspection checks vector
targets, the linked sequencer-to-adapter call path, retained adapter symbols,
and section budgets.

The backend-dispatch check is intentionally pinned to the emitted Cortex-M0
shape. It requires the stopped-state guard to load the flag at offset 6, test
bit 31, compare with zero, and branch on `bne` to the stack teardown. It then
requires the saved variant load and byte extension, compare against 3, and
`bhi` to the non-table placeholder path. That path must return to the teardown
before the ordered four-entry indexed table dispatch. Any other conditional
branch or a changed guard input or destination is rejected; this is a
structural witness check, not a general-purpose Thumb control-flow analyzer.

Run the host test and link-only witness from the repository root:

```sh
cargo test --locked --test board_startup_adapters
RUSTFLAGS='-D warnings -C link-arg=-Tlinker/startup-witness.ld -C link-arg=-Map=../target/startup-witness/startup-adapter-witness.map' \
  cargo build --locked \
  --manifest-path startup-witness/Cargo.toml \
  --target thumbv6m-none-eabi \
  --release \
  --target-dir target/startup-witness
python3 scripts/check_startup_witness.py \
  --elf target/startup-witness/thumbv6m-none-eabi/release/startup-adapter-witness
```

The witness ELF and map are local link-inspection outputs under the ignored
`target/` directory. CI does not convert or upload them, and the witness must
never be executed on hardware. The existing firmware build, startup, and
published-image steps are unchanged.

## Default bootstrap parity

The default bootstrap was rebuilt at the PR base and at the startup-witness
revision below using Rust/Cargo 1.99.1, GNU ARM binutils 2.42, Python 3.12.3,
and identical pack metadata (`UV-K5`, version `dev`). Both source revisions
produce byte-identical ELF, raw, and packed files:

| Source | Commit |
| --- | --- |
| PR base | `dd57ac6a8f605808b8b54049afffef86ffe0f33d` |
| Startup witness | `d9683ba8ada802c74510d46e4d1f1e78d06c8d76` |

| Output | Bytes | SHA-256 at both commits |
| --- | ---: | --- |
| Cortex-M0 ELF | 81,092 | `8544956cb9e90ee8069f5f2d087ecec039d30ea8ab975dd723b98aba0d5f5657` |
| Raw objcopy payload | 300 | `febbcabeb65d724e573494cf64ef6dd6e0bd96a5e2ab3aaf9423030b87c25dbd` |
| Packed container (`UV-K5`, `dev`) | 318 | `0b9f64b6e28a243046cd42efd6ac22774fc4d13b7f8df71c66e1b3f2b86eb8df` |

Reproduce the comparison from a checkout containing both commits:

```sh
set -eu
comparison=$(mktemp -d)
trap 'rm -rf "$comparison"' EXIT
base=dd57ac6a8f605808b8b54049afffef86ffe0f33d
witness=d9683ba8ada802c74510d46e4d1f1e78d06c8d76

for revision in "$base" "$witness"; do
  dir="$comparison/$revision"
  mkdir -p "$dir"
  git archive "$revision" | tar -x -C "$dir"
  (
    cd "$dir"
    cargo rustc --locked --target thumbv6m-none-eabi --release \
      --bin uv-k5-rust-firmware --target-dir target -- -D warnings
    out=target/thumbv6m-none-eabi/release
    arm-none-eabi-objcopy -O binary \
      "$out/uv-k5-rust-firmware" "$out/uv-k5-rust-firmware.raw"
    python3 scripts/firmware_container.py pack \
      "$out/uv-k5-rust-firmware.raw" "$out/uv-k5-rust-firmware.packed" \
      --author UV-K5 --version dev
  )
done

for file in uv-k5-rust-firmware uv-k5-rust-firmware.raw uv-k5-rust-firmware.packed; do
  cmp "$comparison/$base/target/thumbv6m-none-eabi/release/$file" \
    "$comparison/$witness/target/thumbv6m-none-eabi/release/$file"
  sha256sum \
    "$comparison/$base/target/thumbv6m-none-eabi/release/$file" \
    "$comparison/$witness/target/thumbv6m-none-eabi/release/$file"
done
```

These are parity measurements of the unvalidated bootstrap, not witness
artifacts or flashable firmware.
