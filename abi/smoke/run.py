#!/usr/bin/env python3
import argparse
import hashlib
import re
import shlex
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HARNESS = ROOT / "abi" / "smoke"
RUST_SOURCE = HARNESS / "channel_validity.rs"
C_SOURCE = HARNESS / "channel_validity.c"
LINKER_SCRIPT = HARNESS / "linker.ld"
TARGET = "thumbv6m-none-eabi"


def tool(name):
    found = shutil.which(name)
    if not found and name.startswith("llvm-"):
        candidates = [
            path
            for pattern in (f"/usr/bin/{name}-[0-9]*", f"/usr/lib/llvm-*/bin/{name}")
            for path in Path("/").glob(pattern.lstrip("/"))
        ]
        if candidates:
            def version(path):
                match = re.search(rf"{re.escape(name)}-(\d+)$", path.name)
                if match is None:
                    match = re.search(r"/llvm-(\d+)/bin/", path.as_posix())
                return int(match.group(1)) if match else 0

            found = str(max(candidates, key=version))
    if not found:
        raise RuntimeError(f"required tool not found: {name}")
    return found


def run(command, *, cwd=ROOT):
    print("+", shlex.join(map(str, command)), flush=True)
    return subprocess.run(command, cwd=cwd, check=True, text=True, capture_output=True)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def section_sizes(size_output):
    sizes = {}
    for line in size_output.splitlines():
        match = re.match(r"^(\.\S+)\s+(\d+)\s+", line)
        if match:
            sizes[match.group(1)] = int(match.group(2))
    return sizes


def build(output_dir, report_path):
    output_dir.mkdir(parents=True, exist_ok=True)
    rustc = tool("rustc")
    clang = tool("clang")
    objdump = tool("llvm-objdump")
    nm = tool("llvm-nm")
    objcopy = tool("llvm-objcopy")
    size = tool("llvm-size")

    host_rust_object = output_dir / "host-rust.o"
    host_c_object = output_dir / "host-c.o"
    host_executable = output_dir / "host-smoke"
    run([
        rustc, "--edition=2024", "--crate-type=lib", "--emit=obj",
        "-C", "panic=abort", "-C", "opt-level=2", RUST_SOURCE,
        "-o", host_rust_object,
    ])
    run([
        clang, "-std=c11", "-Wall", "-Wextra", "-Werror", "-O2",
        "-c", C_SOURCE, "-o", host_c_object,
    ])
    run([clang, host_c_object, host_rust_object, "-o", host_executable])
    run([host_executable])

    target_rust_object = output_dir / "target-rust.o"
    target_c_object = output_dir / "target-c.o"
    target_elf = output_dir / "mixed-link.elf"
    target_raw = output_dir / "mixed-link.bin"
    target_map = output_dir / "mixed-link.map"
    run([
        rustc, "--edition=2024", "--target", TARGET, "--crate-type=lib",
        "--emit=obj", "-C", "panic=abort", "-C", "opt-level=s",
        RUST_SOURCE, "-o", target_rust_object,
    ])
    run([
        clang, "--target=arm-none-eabi", "-mcpu=cortex-m0", "-mthumb",
        "-ffreestanding", "-fno-builtin", "-ffunction-sections",
        "-fdata-sections", "-std=c11", "-Wall", "-Wextra", "-Werror",
        "-Os", "-c", C_SOURCE, "-o", target_c_object,
    ])
    run([
        clang, "--target=arm-none-eabi", "-mcpu=cortex-m0", "-mthumb",
        "-fuse-ld=lld", "-nostdlib", target_c_object, target_rust_object,
        f"-Wl,-T,{LINKER_SCRIPT}", f"-Wl,-Map,{target_map}",
        "-Wl,--gc-sections", "-Wl,--no-undefined", "-Wl,--build-id=none",
        "-o", target_elf,
    ])
    run([objcopy, "-O", "binary", target_elf, target_raw])
    budget_report = run([
        sys.executable,
        ROOT / "scripts" / "check_memory_budget.py",
        "--elf", target_elf,
        "--raw", target_raw,
        "--stack-reservation", "0x800",
    ]).stdout

    symbols = run([nm, "--defined-only", target_elf]).stdout
    symbol_addresses = {}
    for symbol in ("main", "uvk5_channel_validity"):
        match = re.search(rf"^([0-9a-fA-F]+)\s+\w\s+{symbol}$", symbols, re.MULTILINE)
        if match is None:
            raise RuntimeError(f"linked ELF is missing the {symbol} symbol")
        symbol_addresses[symbol] = int(match.group(1), 16)

    disassembly = run([objdump, "-d", "--no-show-raw-insn", target_elf]).stdout
    main_match = re.search(
        r"<main>:\n(?P<body>.*?)(?=\n[0-9a-f]+ <|\Z)", disassembly, re.DOTALL
    )
    if main_match is None or "uvk5_channel_validity" not in main_match.group("body"):
        raise RuntimeError("target C main does not retain a call to the Rust export")
    if not re.search(r"\bblx?\b", main_match.group("body")):
        raise RuntimeError("target C main has no retained ARM/Thumb call instruction")

    if not target_map.is_file():
        raise RuntimeError("linker map was not generated")
    map_text = target_map.read_text()
    if any(symbol not in map_text for symbol in ("main", "uvk5_channel_validity")):
        raise RuntimeError("linker map does not resolve both C and Rust symbols")
    sizes = section_sizes(run([size, "-A", target_elf]).stdout)
    if ".text" not in sizes:
        raise RuntimeError("target ELF has no linked text section")
    flash = sum(
        value
        for name, value in sizes.items()
        if name in (".text", ".rodata", ".ARM.exidx", ".data")
    )
    ram = sum(value for name, value in sizes.items() if name in (".data", ".bss"))
    static_ram_budget = 16 * 1024 - 2 * 1024
    if flash > 60 * 1024 or ram > static_ram_budget:
        raise RuntimeError(f"target link exceeds flash or static RAM budget: flash={flash}, RAM={ram}")

    report = "\n".join([
        "# Mixed-link ABI smoke evidence",
        "",
        "Generated by `python3 abi/smoke/run.py --report abi/smoke/evidence/target-link-report.md`.",
        "The host executable ran the C test cases against the Rust implementation and exited 0.",
        "The target ELF was linked for `thumbv6m-none-eabi` with a Cortex-M0 C caller and Rust implementation.",
        "This is linked-target evidence only: the target ELF was not executed on hardware or in an emulator.",
        "",
        "## Target link",
        "",
        f"- ELF SHA-256: `{digest(target_elf)}`",
        f"- Map SHA-256: `{digest(target_map)}`",
        f"- Resolved symbols: `main` (C caller, `0x{symbol_addresses['main']:08x}`), `uvk5_channel_validity` (Rust export, `0x{symbol_addresses['uvk5_channel_validity']:08x}`).",
        "- Retained call: `main` disassembly contains a Thumb `bl`/`blx` to `uvk5_channel_validity`.",
        f"- Flash used: {flash} / 61,440 bytes ({60 * 1024 - flash} remaining).",
        f"- Static RAM used: {ram} / {static_ram_budget:,} bytes ({static_ram_budget - ram} remaining); 2,048 bytes are reserved for stack (not measured).",
        f"- Raw payload SHA-256: `{digest(target_raw)}`",
        "",
        "Project memory-budget checker output:",
        "",
        "```text",
        budget_report.rstrip(),
        "```",
        "",
        "## Device results",
        "",
        "Not applicable: this offline proof makes no device changes and performs no on-device execution.",
        "",
        "## Reproduction",
        "",
        "Run `python3 -m unittest discover -s tests -p 'test_abi_smoke.py' -v` to rebuild and validate the host and target artifacts.",
        "",
    ])
    if report_path:
        report_path.parent.mkdir(parents=True, exist_ok=True)
        report_path.write_text(report)
    print(report, end="")


def main():
    parser = argparse.ArgumentParser(description="Build and validate the isolated Rust/C ABI smoke harness.")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "target" / "abi-smoke")
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    report_path = args.report
    if report_path and not report_path.is_absolute():
        report_path = ROOT / report_path
    try:
        build(args.output_dir if args.output_dir.is_absolute() else ROOT / args.output_dir, report_path)
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"ABI smoke failed: {error}", file=sys.stderr)
        if isinstance(error, subprocess.CalledProcessError) and error.stderr:
            print(error.stderr, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
