#!/usr/bin/env python3
"""Inspect the linked, non-flash-addressed Cortex-M0 startup adapter witness."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import re
import subprocess
import sys


class WitnessError(Exception):
    pass


def run_tool(name: str, *arguments: str) -> str:
    prefix = os.environ.get("ARM_BINUTILS_PREFIX", "arm-none-eabi-")
    try:
        result = subprocess.run(
            [f"{prefix}{name}", *arguments],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise WitnessError(f"could not run {name}: {error}") from error
    return result.stdout


def parse_sections(output: str) -> dict[str, tuple[int, int, int]]:
    sections = {}
    for line in output.splitlines():
        match = re.match(
            r"\s*\d+\s+(\S+)\s+([0-9a-fA-F]+)\s+([0-9a-fA-F]+)\s+([0-9a-fA-F]+)",
            line,
        )
        if match:
            name, size, vma, lma = match.groups()
            sections[name] = (int(size, 16), int(vma, 16), int(lma, 16))
    return sections


def parse_symbols(output: str) -> dict[str, int]:
    symbols = {}
    for line in output.splitlines():
        match = re.match(r"^([0-9a-fA-F]+)\s+\w\s+(.+)$", line)
        if match:
            address, name = match.groups()
            symbols[name.strip()] = int(address, 16)
    return symbols


def vector_bytes(output: str) -> bytes:
    contents = output.split("Contents of section .vector_table:", 1)
    if len(contents) != 2:
        raise WitnessError("ELF is missing .vector_table contents")

    data = bytearray()
    for line in contents[1].splitlines():
        match = re.match(r"^\s*[0-9a-fA-F]+\s+((?:[0-9a-fA-F]{8}\s*)+)", line)
        if match is None:
            continue
        data.extend(bytes.fromhex("".join(match.group(1).split())))
    return bytes(data)


def function_block(disassembly: str, symbol_fragment: str, *, suffix: bool = False) -> str:
    labels = list(
        re.finditer(r"(?m)^[0-9a-fA-F]+\s+<(.+)>:\s*$", disassembly)
    )
    for index, label in enumerate(labels):
        if (
            label.group(1).endswith(symbol_fragment)
            if suffix
            else symbol_fragment in label.group(1)
        ):
            end = labels[index + 1].start() if index + 1 < len(labels) else len(disassembly)
            return disassembly[label.end() : end]
    raise WitnessError(f"disassembly is missing {symbol_fragment}")


def validate(elf: Path) -> tuple[int, int]:
    sections = parse_sections(run_tool("objdump", "-h", str(elf)))
    required_sections = {".vector_table", ".text"}
    missing = required_sections - sections.keys()
    if missing:
        raise WitnessError(f"ELF is missing sections: {', '.join(sorted(missing))}")

    vector_size, vector_vma, _ = sections[".vector_table"]
    if vector_size != 0xC0 or vector_vma != 0x1000_0000:
        raise WitnessError("vector table must be 48 words at synthetic address 0x10000000")

    section = lambda name: sections.get(name, (0, 0, 0))
    flash_used = sum(
        section(name)[0]
        for name in (".vector_table", ".text", ".ARM.extab", ".ARM.exidx", ".data")
    )
    ram_used = section(".data")[0] + section(".bss")[0]
    if flash_used > 16 * 1024 or ram_used > 4 * 1024:
        raise WitnessError(
            f"link budget exceeded: {flash_used} bytes WITNESS, {ram_used} bytes RAM"
        )
    for name in (".vector_table", ".text"):
        size, vma, _ = sections[name]
        if size and not 0x1000_0000 <= vma < 0x1000_4000:
            raise WitnessError(f"{name} is outside synthetic WITNESS memory")
    for name in (".ARM.extab", ".ARM.exidx"):
        size, vma, _ = section(name)
        if size and not 0x1000_0000 <= vma < 0x1000_4000:
            raise WitnessError(f"{name} is outside synthetic WITNESS memory")
    data_size, data_vma, data_lma = section(".data")
    if data_size and not (
        0x2001_0000 <= data_vma < 0x2001_1000
        and 0x1000_0000 <= data_lma < 0x1000_4000
    ):
        raise WitnessError(".data is outside the witness RAM/load regions")
    bss_size, bss_vma, _ = section(".bss")
    if bss_size and not 0x2001_0000 <= bss_vma < 0x2001_1000:
        raise WitnessError(".bss is outside the witness RAM region")

    symbols_output = run_tool("nm", "-n", "-C", str(elf))
    symbols = parse_symbols(symbols_output)
    required_symbols = (
        "Reset_Handler",
        "SysTick_Handler",
        "witness_main",
        "board_sequence::run",
        "apply_device_clock_gates",
        "apply_to_hardware",
        "apply_portcon_startup_plan",
        "apply_gpio_startup_plan",
    )
    for symbol in required_symbols:
        if not any(symbol in name for name in symbols):
            raise WitnessError(f"ELF does not retain {symbol}")

    words = vector_bytes(run_tool("objdump", "-s", "-j", ".vector_table", str(elf)))
    if len(words) != 0xC0:
        raise WitnessError("could not read all 48 vector words")
    vector = [int.from_bytes(words[offset : offset + 4], "little") for offset in range(0, 0xC0, 4)]
    if vector[0] != 0x2000_3FF0:
        raise WitnessError("witness vector has an unexpected initial stack pointer")
    for slot, symbol in ((1, "Reset_Handler"), (15, "SysTick_Handler")):
        target = next(address for name, address in symbols.items() if symbol in name)
        if vector[slot] != (target | 1):
            raise WitnessError(f"vector {slot} does not point to {symbol} in Thumb state")

    disassembly = run_tool("objdump", "-d", "-C", str(elf))
    reset = function_block(disassembly, "Reset_Handler")
    if "witness_main" not in reset:
        raise WitnessError("Reset_Handler does not retain the witness startup call path")
    if "bx\tlr" not in function_block(disassembly, "SysTick_Handler"):
        raise WitnessError("SysTick_Handler must return immediately")
    hardware_run = function_block(disassembly, "run_hardware")
    adapter_run = function_block(disassembly, "board_startup_adapters::run::<")
    if "board_startup_adapters::run::<" not in hardware_run:
        raise WitnessError("witness hardware entry does not call the startup adapter runner")
    if "board_sequence::run::<" not in adapter_run:
        raise WitnessError("startup adapter runner does not call the board sequencer")
    sequence = function_block(disassembly, "board_sequence::run")
    sequencer_calls = []
    sequence_lines = sequence.splitlines()
    for index, line in enumerate(sequence_lines[:-1]):
        call = re.search(r"\bmovs\s+r1,\s*#(\d+)", line)
        if call and re.search(r"\bbl\b.*SequencerBackend", sequence_lines[index + 1]):
            sequencer_calls.append(int(call.group(1)))
    if sequencer_calls[:7] != list(range(7)):
        raise WitnessError("linked sequencer does not retain the ordered startup call dispatches")
    for method, adapter in (
        ("device_clock_gates", "apply_device_clock_gates"),
        ("systick", "apply_to_hardware"),
        ("portcon", "apply_portcon_startup_plan"),
        ("gpio", "apply_gpio_startup_plan"),
    ):
        method_body = function_block(disassembly, f"StartupIo>::{method}", suffix=True)
        if adapter not in method_body:
            raise WitnessError(f"target {method} adapter is not wired to {adapter}")

    return flash_used, ram_used


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--elf", required=True, type=Path)
    arguments = parser.parse_args(argv)
    try:
        flash_used, ram_used = validate(arguments.elf)
    except (OSError, WitnessError, StopIteration) as error:
        print(f"startup witness check failed: {error}", file=sys.stderr)
        return 1
    print(
        "Startup witness linked and inspected: 48-word reset/SysTick vectors, "
        "adapter call path retained, "
        f"{flash_used}/16384 witness bytes, {ram_used}/4096 RAM bytes"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
