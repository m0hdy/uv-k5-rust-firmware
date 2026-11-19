#!/usr/bin/env python3
import argparse
import hashlib
import os
import re
import shlex
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HARNESS = ROOT / "mixed-fallback"
TARGET = "thumbv6m-none-eabi"
SELECTORS = {
    "c": ("mixed-fallback-c", "mixed_c_channel_valid", "mixed_rust_channel_valid"),
    "rust": ("mixed-fallback-rust", "mixed_rust_channel_valid", "mixed_c_channel_valid"),
}
INPUTS = (
    "Cargo.toml",
    ".cargo/config.toml",
    "build.rs",
    "mixed-fallback/fallback.c",
    "mixed-fallback/compare.c",
    "mixed-fallback/rust_impl.rs",
    "mixed-fallback/run.py",
    "src/bin/mixed_fallback.rs",
    "src/bin/startup.S",
    "linker/firmware.ld",
    "scripts/check_startup_layout.py",
    "scripts/check_memory_budget.py",
    "tests/test_mixed_fallback.py",
)


def tool(name):
    found = shutil.which(name)
    if not found:
        raise RuntimeError(f"required tool not found: {name}")
    return found


def run(command, *, cwd=ROOT, env=None):
    print("+", shlex.join(map(str, command)), flush=True)
    return subprocess.run(
        command, cwd=cwd, env=env, check=True, text=True, capture_output=True
    )


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def tool_version(command):
    result = run(command)
    return (result.stdout or result.stderr).splitlines()[0]


def compare_host_outcomes(output_dir, rustc, clang):
    rust_object = output_dir / "host-rust.o"
    fallback_object = output_dir / "host-fallback.o"
    host_test = output_dir / "host-compare"
    run([
        rustc,
        "--edition=2024",
        "--crate-type=lib",
        "--emit=obj",
        "-C",
        "opt-level=2",
        HARNESS / "rust_impl.rs",
        "-o",
        rust_object,
    ])
    run([
        clang,
        "-std=c11",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-O2",
        "-c",
        HARNESS / "fallback.c",
        "-o",
        fallback_object,
    ])
    run([
        clang,
        "-std=c11",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-O2",
        HARNESS / "compare.c",
        fallback_object,
        rust_object,
        "-o",
        host_test,
    ])
    run([host_test])


def defined_symbols(nm, elf):
    output = run([nm, "--defined-only", elf]).stdout
    return {line.split()[-1] for line in output.splitlines() if line.split()}


def validate_target(name, selected, discarded, artifacts, tools):
    elf = artifacts / f"fallback-{name}.elf"
    map_file = artifacts / f"fallback-{name}.map"
    raw = artifacts / f"fallback-{name}.bin"
    map_text = map_file.read_text(encoding="utf-8")
    symbols = defined_symbols(tools["nm"], elf)
    if selected not in symbols or discarded in symbols:
        raise RuntimeError(
            f"{name} link symbols do not prove exclusive selection: "
            f"{selected} present={selected in symbols}, "
            f"{discarded} present={discarded in symbols}"
        )
    if selected not in map_text:
        raise RuntimeError(f"{name} linker map omits selected implementation {selected}")

    disassembly = run([tools["objdump"], "-d", elf]).stdout
    main = re.search(
        r"<firmware_main>:\n(?P<body>.*?)(?=\n[0-9a-fA-F]+ <|\Z)",
        disassembly,
        re.DOTALL,
    )
    if main is None or not re.search(
        rf"\bblx?\b[^\n]*<{re.escape(selected)}>", main.group("body")
    ):
        raise RuntimeError(
            f"{name} firmware_main disassembly does not call selected {selected}"
        )

    startup = run([
        sys.executable,
        ROOT / "scripts" / "check_startup_layout.py",
        "--elf",
        elf,
        "--map",
        map_file,
    ]).stdout
    budget = run([
        sys.executable,
        ROOT / "scripts" / "check_memory_budget.py",
        "--elf",
        elf,
        "--raw",
        raw,
        "--stack-reservation",
        "0x800",
    ]).stdout
    return {
        "elf": elf,
        "map": map_file,
        "raw": raw,
        "symbols": symbols,
        "startup": startup,
        "budget": budget,
        "flash": re.search(r"Flash load bytes: (\d+) / (\d+)", budget).groups(),
        "ram": re.search(r"RAM static use: (\d+) bytes", budget).group(1),
        "raw_bytes": raw.stat().st_size,
    }


def build_target(output_dir, cargo, tools):
    artifacts = output_dir / "target"
    artifacts.mkdir(parents=True, exist_ok=True)
    cargo_env = os.environ.copy()
    cargo_env["CARGO_TARGET_DIR"] = str(artifacts.resolve())
    results = {}
    for name, (feature, selected, discarded) in SELECTORS.items():
        map_file = artifacts / f"fallback-{name}.map"
        cargo_args = [
            cargo,
            "rustc",
            "--locked",
            "--target",
            TARGET,
            "--release",
            "--bin",
            "mixed-fallback",
            "--features",
            f"mixed-fallback,{feature}",
            "--",
            "-D",
            "warnings",
            "-C",
            f"link-arg=-Map={map_file}",
            "-C",
            "link-arg=--gc-sections",
        ]
        run(cargo_args, env=cargo_env)
        cargo_elf = artifacts / TARGET / "release" / "mixed-fallback"
        elf = artifacts / f"fallback-{name}.elf"
        shutil.copyfile(cargo_elf, elf)
        run([tools["objcopy"], "-O", "binary", elf, artifacts / f"fallback-{name}.bin"])
        results[name] = validate_target(
            name, selected, discarded, artifacts, tools
        )
    return results


def source_hashes():
    return {path: digest(ROOT / path) for path in INPUTS}


def make_report(results, versions):
    try:
        revision = run(["git", "rev-parse", "HEAD"]).stdout.strip()
    except subprocess.CalledProcessError:
        revision = "unavailable"
    rows = []
    for name, result in results.items():
        rows.extend([
            f"### {name.upper()} selected",
            "",
            f"- Retained implementation: `{SELECTORS[name][1]}`",
            f"- ELF SHA-256: `{digest(result['elf'])}`",
            f"- Map SHA-256: `{digest(result['map'])}`",
            f"- Raw SHA-256: `{digest(result['raw'])}`",
            f"- Raw bytes: {result['raw_bytes']}",
            f"- Flash load: {result['flash'][0]} / {result['flash'][1]} bytes.",
            f"- Static RAM: {result['ram']} bytes; 2,048 bytes reserved for stack.",
            "- Startup layout and exact ELF/raw budget checks: passed.",
            "",
            "Startup check:",
            "",
            "```text",
            result["startup"].rstrip(),
            "```",
            "",
            "Memory budget check:",
            "",
            "```text",
            result["budget"].rstrip(),
            "```",
            "",
        ])
    hashes = "\n".join(f"- `{path}`: `{value}`" for path, value in source_hashes().items())
    return "\n".join([
        "# Mixed-fallback image evidence",
        "",
        "Generated by `python3 mixed-fallback/run.py --report mixed-fallback/evidence.md`.",
        "The host comparison exercised seven normal and boundary channel values against both implementations.",
        "Both target selections use `src/bin/startup.S` and `linker/firmware.ld`; the unselected operation is absent from the linked ELF symbol table.",
        "No target execution, device use, release artifact, or hardware dependency closure is claimed.",
        "",
        "## Source and toolchain",
        "",
        f"- Git HEAD (base revision): `{revision}`; source input hashes below identify the exact working-tree inputs.",
        f"- Rust: `{versions['rustc']}`",
        f"- Cargo: `{versions['cargo']}`",
        f"- Clang: `{versions['clang']}`",
        f"- ARM binutils: `{versions['binutils']}`",
        "- Opt-in target: `thumbv6m-none-eabi` / Cortex-M0.",
        "",
        "Source SHA-256:",
        "",
        hashes,
        "",
        "## Target selections",
        "",
        *rows,
        "## Reproduction",
        "",
        "From the repository root, with the pinned Rust target, Clang, GNU ARM binutils and Python 3 available:",
        "",
        "```sh",
        "UVK5_RUN_MIXED_FALLBACK=1 python3 -m unittest discover -s tests -p 'test_mixed_fallback.py' -v",
        "python3 mixed-fallback/run.py --report mixed-fallback/evidence.md",
        "```",
        "",
        "The runner compares host results, links each explicit feature selection with Cargo's target directory pinned under the output directory, checks map and disassembly call retention, runs the startup-layout validator, creates the raw payload with ARM `objcopy`, and checks the exact ELF/raw pair with a 2 KiB stack reservation.",
        "",
        "## Device results",
        "",
        "Not run. These artifacts are offline software evidence and are not release images.",
        "",
    ])


def build(output_dir, report_path):
    output_dir.mkdir(parents=True, exist_ok=True)
    cargo = tool("cargo")
    rustc = tool("rustc")
    clang = tool(os.environ.get("CLANG", "clang"))
    tools = {
        "nm": tool("arm-none-eabi-nm"),
        "objdump": tool("arm-none-eabi-objdump"),
        "objcopy": tool("arm-none-eabi-objcopy"),
    }
    compare_host_outcomes(output_dir, rustc, clang)
    results = build_target(output_dir, cargo, tools)
    versions = {
        "rustc": tool_version([rustc, "--version"]),
        "cargo": tool_version([cargo, "--version"]),
        "clang": tool_version([clang, "--version"]),
        "binutils": tool_version([tools["objcopy"], "--version"]),
    }
    report = make_report(results, versions)
    if report_path:
        report_path.parent.mkdir(parents=True, exist_ok=True)
        report_path.write_text(report, encoding="utf-8")
    print(report, end="")


def main():
    parser = argparse.ArgumentParser(
        description="Build and validate both opt-in mixed-fallback image selections."
    )
    parser.add_argument(
        "--output-dir", type=Path, default=ROOT / "target" / "mixed-fallback"
    )
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    output_dir = args.output_dir if args.output_dir.is_absolute() else ROOT / args.output_dir
    report_path = args.report
    if report_path and not report_path.is_absolute():
        report_path = ROOT / report_path
    try:
        build(output_dir, report_path)
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"Mixed-fallback build failed: {error}", file=sys.stderr)
        if isinstance(error, subprocess.CalledProcessError) and error.stderr:
            print(error.stderr, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
