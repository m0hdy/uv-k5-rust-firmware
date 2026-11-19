# Board startup adapter witness

`src/board_startup_adapters.rs` dispatches the existing sequencer into the
clock-gate, SysTick, PortCon, GPIO, and backlight target adapters. `ENABLE_SWD`
is forwarded to PortCon and `ENABLE_FMRADIO` to GPIO. Backlight initialization
calls the existing `backlight_startup_executor::initialize_backlight_hardware`
after GPIO and before ADC/display; ADC, display, BK1080, and CRC remain typed
placeholders. A failed SysTick setup stops further adapter calls before
PortCon, GPIO, or backlight can run.

The host test checks the composed call order, feature forwarding, backlight
register sequence, fresh read-modify-write behavior, preservation of unrelated
clock and PORTB bits, and stop behavior. Target execution requires the
DP32G030 PWM_PLUS0 and PORTB register mappings, the PWM_PLUS0 clock gate
enabled by the earlier clock-gate stage, B6 reserved for the backlight and
selected for PWMP0_CH0 by the backlight mux update, and exclusive access to
PWM_PLUS0 CLKSRC, PERIOD, GEN, CFG and PORTB SEL0 for the complete sequence.
The host and link-only checks do not establish those hardware preconditions.

CI separately links `startup-witness/` for Cortex-M0 with a dedicated
vector table and a returning SysTick handler. Its linker places code at
`0x10000000` and RAM at `0x20010000`, outside the DP32G030 memory map, and
enforces 16 KiB code/load and 4 KiB RAM limits. The inspection checks vector
targets, the linked sequencer-to-adapter call path, reachable backlight
register accesses and their order, retained adapter symbols, and section
budgets. The SysTick vector must match the exact linked handler symbol, whose
first instruction must be `bx lr`.

The backend-dispatch check is intentionally pinned to the emitted Cortex-M0
shape. It requires the function entry to save incoming `r1` at `sp+8`, load
that saved variant, byte-extend and save the checked index, compare against 4,
and branch with `bhi` to the non-table placeholder path. The path must return
to the stack teardown before the contiguous indexed-table sequence reloads the
saved index from `sp+0`, scales it, addresses the literal table, and branches
through its entry. Each of the first five cases is checked through its matching
hardware call and the following unconditional branch to the shared stack
teardown. The backlight case must reach the real executor and preserve the
pinned register sequence. The SysTick case also checks the `Result::is_err`
split: success continues to teardown, while the error edge stores the stopped
flag before joining that same teardown. The sequencer's first seven backend
calls are checked for their call continuations and the optional FM-radio and
CRC paths are checked through their shared return epilogue. The `run_hardware`
wrapper and all five target wrappers are checked from their linked calls
through stack cleanup and return. The adapter's `Result` split is checked so
both success and error edges reach its shared return. Reset and `witness_main`
retain their intentional terminal loops and are not treated as returning
wrappers.
Unexpected instructions, conditional branches, or changed guard inputs,
destinations, and continuations are rejected. This is linked structural
evidence for the emitted paths; it is not a general-purpose Thumb control-flow
analyzer and does not establish that arbitrary callees or MMIO operations
complete.

Run focused host tests, target checks, and link-only witness inspection from
the repository root:

```sh
cargo test --locked --test board_startup_adapters
cargo test --locked --test boot_coordinator
cargo clippy --locked --test board_startup_adapters --test boot_coordinator \
  --test board_startup_composition --test backlight_startup_executor -- -D warnings
RUSTFLAGS='-D warnings' cargo check --locked --lib --target thumbv6m-none-eabi
python3 -m unittest tests.test_startup_witness
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
