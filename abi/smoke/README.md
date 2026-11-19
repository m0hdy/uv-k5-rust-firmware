# Mixed-link ABI smoke harness

This isolated witness links a C caller to a C-ABI Rust wrapper around the
existing pure Rust `channel_validity::is_valid_channel` routine. It does not
add a production firmware API or change the firmware startup/linker.

The wrapper uses the fixed-width types required by
[`abi-contract.md`](../../docs/abi-contract.md). It returns status `0` and
writes `0` or `1` to `out_valid`; invalid pointers or boolean flag values
outside `0..=1` return status `1` without changing the output. The C table
covers normal and boundary channels, bands, scan-list behavior, priority
exclusions, and invalid arguments.

Run the host comparison and target-link inspection with:

```sh
python3 -m unittest discover -s tests -p 'test_abi_smoke.py' -v
```

The test needs `rustc`, Clang with LLD, and LLVM `nm`, `objdump`, and `size`
tools. It builds in a temporary directory. To regenerate the checked-in
evidence report and default ignored artifacts:

```sh
python3 abi/smoke/run.py --report abi/smoke/evidence/target-link-report.md
```

The host C executable is run and its results are checked. The
`thumbv6m-none-eabi` output is linked and inspected for symbol resolution,
the retained Thumb call, and use of the target contract's 60 KiB flash and
16 KiB RAM region, reserving 2 KiB for stack. The reservation is a budget
allowance, not a measured stack high-water mark. The target ELF is not
executed on hardware or in an emulation environment and is not a firmware
image.
