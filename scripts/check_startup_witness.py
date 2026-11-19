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


def direct_call_targets(function: str) -> list[str]:
    return [
        match.group(1)
        for line in function.splitlines()
        if (match := re.search(r"\bbl(?:x)?\s+[0-9a-fA-F]+\s+<(.+)>$", line))
    ]


def parse_instructions(function: str) -> list[tuple[int, int, str, str, str]]:
    instructions = []
    for line in function.splitlines():
        if "\t" in line:
            parts = line.split("\t", 2)
            address_match = re.match(r"\s*([0-9a-fA-F]+):", parts[0])
            instruction_match = (
                re.match(r"\s*([a-z][\w.]*)\s*(.*)", parts[2])
                if len(parts) == 3
                else None
            )
            if address_match is None or instruction_match is None:
                continue
            address = address_match.group(1)
            encoded = parts[1]
            mnemonic, operands = instruction_match.groups()
        else:
            match = re.match(
                r"\s*([0-9a-fA-F]+):\s+((?:(?:[0-9a-fA-F]{4}|[0-9a-fA-F]{2})\s+)+)([a-z][\w.]*)\s*(.*)",
                line,
            )
            if match is None:
                continue
            address, encoded, mnemonic, operands = match.groups()
        if mnemonic != ".word":
            size = sum(len(byte) // 2 for byte in encoded.split())
            instructions.append(
                (int(address, 16), size, mnemonic, operands, line)
            )
    return sorted(instructions)


def is_control_transfer(
    mnemonic: str, operands: str, line: str
) -> bool:
    if mnemonic in ("bl", "blx"):
        return not direct_call_targets(line)
    if re.fullmatch(
        r"b(?:\.(?:n|w))?|b(?:eq|ne|cs|hs|cc|lo|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)(?:\.(?:n|w))?",
        mnemonic,
    ) or mnemonic in ("bx", "cbz", "cbnz", "tbb", "tbh"):
        return True
    return bool(re.match(r"pc\s*,", operands)) or (
        (mnemonic == "pop" or mnemonic.startswith("ldm"))
        and re.search(r"\bpc\b", operands) is not None
    )


def reachable_instruction_prefix(function: str) -> list[tuple[int, int, str, str, str]]:
    instructions = parse_instructions(function)
    reachable = []
    next_address = instructions[0][0] if instructions else None
    for instruction in instructions:
        address, size, mnemonic, operands, line = instruction
        if address != next_address:
            break
        reachable.append(instruction)
        next_address = address + size
        if is_control_transfer(mnemonic, operands, line):
            break
    return reachable


def reachable_call_targets(function: str) -> list[str]:
    return [
        target
        for _, _, _, _, line in reachable_instruction_prefix(function)
        for target in direct_call_targets(line)
    ]


def require_call_edge(disassembly: str, caller: str, callee: str) -> None:
    targets = reachable_call_targets(function_block(disassembly, caller))
    if not any(callee in target for target in targets):
        raise WitnessError(
            f"{caller} has no reachable linked call instruction to {callee}"
        )


def backend_dispatch_targets(function: str) -> list[int]:
    instructions = parse_instructions(function)
    prefix = []
    next_address = instructions[0][0] if instructions else None
    dispatch_target = None
    for instruction in instructions:
        address, size, mnemonic, operands, line = instruction
        if address != next_address:
            break
        prefix.append(instruction)
        next_address = address + size
        if re.fullmatch(
            r"b(?:eq|ne|cs|hs|cc|lo|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)(?:\.(?:n|w))?",
            mnemonic,
        ):
            target = re.match(r"\s*([0-9a-fA-F]+)\b", operands)
            if target is None or int(target.group(1), 16) <= address:
                raise WitnessError("sequencer backend dispatch has an unsupported conditional branch")
            continue
        if mnemonic in ("b", "b.n", "b.w"):
            target = re.match(r"\s*([0-9a-fA-F]+)\b", operands)
            if target is None or int(target.group(1), 16) != next_address:
                raise WitnessError("sequencer backend dispatch has an unsupported unconditional branch")
            continue
        if re.search(r"\bmov\s+pc,\s*r0\b", line):
            dispatch_target = address
            break
        if is_control_transfer(mnemonic, operands, line):
            raise WitnessError("sequencer backend dispatch is unreachable from its function entry")

    def position(pattern: str) -> tuple[int, re.Match[str] | None]:
        for index, (_, _, _, _, line) in enumerate(prefix):
            match = re.search(pattern, line)
            if match:
                return index, match
        return len(prefix), None

    scale_index, _ = position(r"\b(?:lsls|lsl)\s+r1,\s*r0,\s*#2\b")
    table_index, table = position(r"\badr\s+r0,\s*([0-9a-fA-F]+)\b")
    if table is None:
        table_index, table = position(
            r"\badd\s+r0,\s*pc,\s*#[0-9]+\b.*?\badr\s+r0,\s*([0-9a-fA-F]+)\b"
        )
    load_index, _ = position(r"\bldr\s+r0,\s*\[r0,\s*r1\]")
    branch_index, _ = position(r"\bmov\s+pc,\s*r0\b")
    if (
        dispatch_target is None
        or scale_index >= table_index
        or table_index >= load_index
        or load_index >= branch_index
        or table_index != scale_index + 1
        or load_index != table_index + 1
        or branch_index != load_index + 1
    ):
        raise WitnessError(
            "sequencer backend has no reachable ordered indexed dispatch from its function entry"
        )

    table_address = int(table.group(1), 16)
    words = {}
    for line in function.splitlines():
        match = re.match(
            r"\s*([0-9a-fA-F]+):\s+(?:[0-9a-fA-F]+\s+)+\.word\s+0x([0-9a-fA-F]+)\b",
            line,
        )
        if match:
            words[int(match.group(1), 16)] = int(match.group(2), 16)
    try:
        return [words[table_address + offset] & ~1 for offset in range(0, 16, 4)]
    except KeyError as error:
        raise WitnessError("sequencer backend dispatch table is missing one of its first four targets") from error


def validate_backend_dispatch(disassembly: str) -> None:
    backend = function_block(disassembly, ">::call", suffix=True)
    targets = backend_dispatch_targets(backend)
    if len(set(targets)) != 4:
        raise WitnessError("sequencer backend first four variants do not have distinct dispatch targets")

    instructions = parse_instructions(backend)
    methods = (
        "StartupIo>::device_clock_gates",
        "StartupIo>::systick",
        "StartupIo>::portcon",
        "StartupIo>::gpio",
    )
    ordered_targets = sorted(targets)
    for variant, (target, method) in enumerate(zip(targets, methods)):
        next_target = next((address for address in ordered_targets if address > target), None)
        block = []
        for instruction in instructions:
            address = instruction[0]
            if target <= address and (next_target is None or address < next_target):
                block.append(instruction)

        address = target
        reachable_call = False
        for instruction_address, size, mnemonic, operands, line in block:
            if instruction_address != address:
                break
            calls = direct_call_targets(line)
            if mnemonic in ("bl", "blx"):
                reachable_call = any(method in call for call in calls)
                break
            if is_control_transfer(mnemonic, operands, line):
                break
            address += size
        if not reachable_call:
            raise WitnessError(
                f"sequencer backend variant {variant} does not reach a direct call to {method}"
            )


def validate_call_path(disassembly: str) -> None:
    for caller, callee in (
        ("Reset_Handler", "witness_main"),
        ("witness_main", "run_hardware"),
        ("run_hardware", "board_startup_adapters::run::<"),
        ("board_startup_adapters::run::<", "board_sequence::run::<"),
    ):
        require_call_edge(disassembly, caller, callee)

    sequence = function_block(disassembly, "board_sequence::run")
    reachable_sequence = reachable_instruction_prefix(sequence)
    sequencer_calls = [
        target
        for _, _, _, _, line in reachable_sequence
        for target in direct_call_targets(line)
    ]
    if sum("Backend>::call" in target for target in sequencer_calls) < 7:
        raise WitnessError(
            "linked sequencer does not retain its reachable ordered startup call dispatches"
        )
    sequencer_call_args = [
        int(argument.group(1))
        for index, (_, _, _, _, line) in enumerate(reachable_sequence[:-1])
        if (argument := re.search(r"\bmovs\s+r1,\s*#(\d+)", line))
        and any(
            "Backend>::call" in target
            for target in direct_call_targets(reachable_sequence[index + 1][4])
        )
    ]
    if sequencer_call_args[:7] != list(range(7)):
        raise WitnessError("linked sequencer startup calls are not in the pinned order")

    for method, adapter in (
        ("device_clock_gates", "apply_device_clock_gates"),
        ("systick", "apply_to_hardware"),
        ("portcon", "apply_portcon_startup_plan"),
        ("gpio", "apply_gpio_startup_plan"),
    ):
        method_body = function_block(disassembly, f"StartupIo>::{method}", suffix=True)
        if not any(adapter in target for target in reachable_call_targets(method_body)):
            raise WitnessError(
                f"target {method} has no reachable linked call instruction to {adapter}"
            )

    validate_backend_dispatch(disassembly)


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
    validate_call_path(disassembly)

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
