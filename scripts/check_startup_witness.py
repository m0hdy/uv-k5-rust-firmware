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


def branch_target(instruction: tuple[int, int, str, str, str]) -> int | None:
    target = re.match(r"\s*([0-9a-fA-F]+)\b", instruction[3])
    return int(target.group(1), 16) if target else None


def matches_instruction(
    instruction: tuple[int, int, str, str, str], mnemonic: str, operands: str
) -> bool:
    return instruction[2] == mnemonic and re.fullmatch(operands, instruction[3]) is not None


def validate_stopped_guard(
    prefix: list[tuple[int, int, str, str, str]],
    branch: tuple[int, int, str, str, str],
    instructions: list[tuple[int, int, str, str, str]],
) -> bool:
    if len(prefix) < 4:
        return False
    load, shift, compare = prefix[-4:-1]
    if not (
        matches_instruction(load, "ldrb", r"\s*r0,\s*\[r0,\s*#6\]\s*")
        and matches_instruction(shift, "lsls", r"\s*r0,\s*r0,\s*#31\s*")
        and matches_instruction(compare, "cmp", r"\s*r0,\s*#0\s*")
        and branch[2] == "bne.n"
    ):
        return False

    target = branch_target(branch)
    if target is None:
        return False
    by_address = {instruction[0]: instruction for instruction in instructions}
    teardown = [by_address.get(target), by_address.get(target + 2)]
    return (
        all(teardown)
        and matches_instruction(teardown[0], "add", r"\s*sp,\s*#\d+\s*")
        and teardown[1][2] == "pop"
        and re.search(r"\bpc\b", teardown[1][3]) is not None
    )


def validate_variant_guard(
    prefix: list[tuple[int, int, str, str, str]],
    branch: tuple[int, int, str, str, str],
    instructions: list[tuple[int, int, str, str, str]],
) -> bool:
    if len(prefix) < 5:
        return False
    load, extend, save, compare = prefix[-5:-1]
    if not (
        matches_instruction(load, "ldr", r"\s*r0,\s*\[sp,\s*#8\]\s*")
        and matches_instruction(extend, "uxtb", r"\s*r0,\s*r0\s*")
        and matches_instruction(save, "str", r"\s*r0,\s*\[sp,\s*#0\]\s*")
        and matches_instruction(compare, "cmp", r"\s*r0,\s*#3\s*")
        and branch[2] == "bhi.n"
    ):
        return False

    target = branch_target(branch)
    if target is None:
        return False
    by_address = {instruction[0]: instruction for instruction in instructions}
    handler = [by_address.get(target + offset) for offset in (0, 2, 4, 6)]
    if not all(handler):
        return False
    call = handler[3]
    return_target = next(
        (
            branch_target(instruction)
            for instruction in prefix
            if instruction[2] == "bne.n"
        ),
        None,
    )
    tail = by_address.get(call[0] + call[1])
    return (
        matches_instruction(handler[0], "ldr", r"\s*r1,\s*\[sp,\s*#8\]\s*")
        and matches_instruction(handler[1], "ldr", r"\s*r0,\s*\[sp,\s*#4\]\s*")
        and matches_instruction(handler[2], "ldr", r"\s*r0,\s*\[r0,\s*#0\]\s*")
        and any("placeholder" in target for target in direct_call_targets(call[4]))
        and tail is not None
        and tail[2] in ("b.n", "b.w")
        and branch_target(tail) == return_target
    )


def require_call_edge(disassembly: str, caller: str, callee: str) -> None:
    targets = reachable_call_targets(function_block(disassembly, caller))
    if not any(callee in target for target in targets):
        raise WitnessError(
            f"{caller} has no reachable linked call instruction to {callee}"
        )


def backend_dispatch_targets(function: str) -> list[int]:
    instructions = parse_instructions(function)
    prefix = instructions[:20]
    if len(prefix) != 20 or any(
        previous[0] + previous[1] != current[0]
        for previous, current in zip(prefix, prefix[1:])
    ):
        raise WitnessError(
            "sequencer backend has no reachable ordered indexed dispatch from its function entry"
        )

    expected = (
        (0, "push", r"\s*\{\s*r7,\s*lr\s*\}\s*"),
        (1, "add", r"\s*r7,\s*sp,\s*#0\s*"),
        (2, "sub", r"\s*sp,\s*#16\s*"),
        (3, "str", r"\s*r0,\s*\[sp,\s*#4\]\s*"),
        (4, "str", r"\s*r1,\s*\[sp,\s*#8\]\s*"),
        (5, "ldrb", r"\s*r0,\s*\[r0,\s*#6\]\s*"),
        (6, "lsls", r"\s*r0,\s*r0,\s*#31\s*"),
        (7, "cmp", r"\s*r0,\s*#0\s*"),
        (8, "bne.n", r"\s*[0-9a-fA-F]+\b.*"),
        (9, "b.n", r"\s*[0-9a-fA-F]+\b.*"),
        (10, "ldr", r"\s*r0,\s*\[sp,\s*#8\]\s*"),
        (11, "uxtb", r"\s*r0,\s*r0\s*"),
        (12, "str", r"\s*r0,\s*\[sp,\s*#0\]\s*"),
        (13, "cmp", r"\s*r0,\s*#3\s*"),
        (14, "bhi.n", r"\s*[0-9a-fA-F]+\b.*"),
        (15, "ldr", r"\s*r0,\s*\[sp,\s*#0\]\s*"),
        (16, "lsls", r"\s*r1,\s*r0,\s*#2\s*"),
        (18, "ldr", r"\s*r0,\s*\[r0,\s*r1\]\s*"),
        (19, "mov", r"\s*pc,\s*r0\s*"),
    )
    if any(
        not matches_instruction(prefix[index], mnemonic, operands)
        for index, mnemonic, operands in expected
    ):
        raise WitnessError(
            "sequencer backend has no reachable ordered indexed dispatch from its function entry"
        )

    stopped_branch = prefix[8]
    variant_branch = prefix[14]
    if (
        not validate_stopped_guard(prefix[:9], stopped_branch, instructions)
        or branch_target(prefix[9]) != prefix[10][0]
        or not validate_variant_guard(prefix[:15], variant_branch, instructions)
    ):
        raise WitnessError(
            "sequencer backend has no reachable ordered indexed dispatch from its function entry"
        )

    table = re.search(
        r"\badd\s+r0,\s*pc,\s*#[0-9]+\b.*?\badr\s+r0,\s*([0-9a-fA-F]+)\b",
        prefix[17][4],
    )
    if table is None:
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
