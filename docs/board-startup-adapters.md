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
