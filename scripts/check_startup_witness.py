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
    by_address = {instruction[0]: instruction for instruction in instructions}
    methods = (
        "StartupIo>::device_clock_gates",
        "StartupIo>::systick",
        "StartupIo>::portcon",
        "StartupIo>::gpio",
    )
    epilogue = branch_target(parse_instructions(backend)[8])
    if epilogue is None:
        raise WitnessError("sequencer backend has no shared return epilogue")

    def expect(
        address: int,
        mnemonic: str,
        operands: str,
        variant: int,
    ) -> int:
        instruction = by_address.get(address)
        if instruction is None or not matches_instruction(instruction, mnemonic, operands):
            raise WitnessError(
                f"sequencer backend variant {variant} has an unexpected continuation"
            )
        return address + instruction[1]

    def expect_call(address: int, method: str, variant: int) -> int:
        instruction = by_address.get(address)
        if (
            instruction is None
            or instruction[2] not in ("bl", "blx")
            or not any(method in target for target in direct_call_targets(instruction[4]))
        ):
            raise WitnessError(
                f"sequencer backend variant {variant} does not reach a direct call to {method}"
            )
        return address + instruction[1]

    def expect_epilogue_branch(address: int, variant: int) -> None:
        instruction = by_address.get(address)
        if (
            instruction is None
            or instruction[2] not in ("b.n", "b.w")
            or branch_target(instruction) != epilogue
        ):
            raise WitnessError(
                f"sequencer backend variant {variant} does not continue to the shared return epilogue"
            )

    epilogue_instructions = [by_address.get(epilogue), by_address.get(epilogue + 2)]
    if not (
        all(epilogue_instructions)
        and matches_instruction(epilogue_instructions[0], "add", r"\s*sp,\s*#16\s*")
        and matches_instruction(epilogue_instructions[1], "pop", r"\s*\{\s*r7,\s*pc\s*\}\s*")
    ):
        raise WitnessError("sequencer backend shared return epilogue has an unexpected shape")

    clock_end = expect(targets[0], "ldr", r"\s*r0,\s*\[sp,\s*#4\]\s*", 0)
    clock_end = expect(clock_end, "ldr", r"\s*r0,\s*\[r0,\s*#0\]\s*", 0)
    clock_end = expect_call(clock_end, methods[0], 0)
    expect_epilogue_branch(clock_end, 0)

    systick_end = expect(targets[1], "ldr", r"\s*r0,\s*\[sp,\s*#4\]\s*", 1)
    systick_end = expect(systick_end, "ldr", r"\s*r0,\s*\[r0,\s*#0\]\s*", 1)
    systick_end = expect(
        systick_end,
        "ldr",
        r"\s*r1,\s*\[pc,\s*#\d+\](?:\s+[@;].*)?\s*",
        1,
    )
    systick_end = expect_call(systick_end, methods[1], 1)
    systick_end = expect(systick_end, "mov", r"\s*r1,\s*r0\s*", 1)
    systick_end = expect(systick_end, "add", r"\s*r0,\s*sp,\s*#12\s*", 1)
    systick_end = expect(systick_end, "strb", r"\s*r1,\s*\[r0,\s*#0\]\s*", 1)
    systick_end = expect_call(
        systick_end,
        "Result<(), startup_adapter_witness::board_startup_adapters::StartupError>>::is_err",
        1,
    )
    systick_end = expect(systick_end, "cmp", r"\s*r0,\s*#0\s*", 1)
    is_err_branch = by_address.get(systick_end)
    if is_err_branch is None or is_err_branch[2] != "bne.n":
        raise WitnessError("sequencer backend SysTick result split has an unexpected branch")
    error_handler = branch_target(is_err_branch)
    systick_end += is_err_branch[1]
    expect_epilogue_branch(systick_end, 1)

    if error_handler is None:
        raise WitnessError("sequencer backend SysTick error branch has no target")
    error_end = expect(error_handler, "ldr", r"\s*r1,\s*\[sp,\s*#4\]\s*", 1)
    error_end = expect(error_end, "movs", r"\s*r0,\s*#1\s*", 1)
    error_end = expect(error_end, "strb", r"\s*r0,\s*\[r1,\s*#6\]\s*", 1)
    expect_epilogue_branch(error_end, 1)

    portcon_end = expect(targets[2], "ldr", r"\s*r1,\s*\[sp,\s*#4\]\s*", 2)
    portcon_end = expect(portcon_end, "ldr", r"\s*r0,\s*\[r1,\s*#0\]\s*", 2)
    portcon_end = expect(portcon_end, "ldrb", r"\s*r1,\s*\[r1,\s*#4\]\s*", 2)
    portcon_end = expect_call(portcon_end, methods[2], 2)
    expect_epilogue_branch(portcon_end, 2)

    gpio_end = expect(targets[3], "ldr", r"\s*r1,\s*\[sp,\s*#4\]\s*", 3)
    gpio_end = expect(gpio_end, "ldr", r"\s*r0,\s*\[r1,\s*#0\]\s*", 3)
    gpio_end = expect(gpio_end, "ldrb", r"\s*r1,\s*\[r1,\s*#5\]\s*", 3)
    gpio_end = expect_call(gpio_end, methods[3], 3)
    expect_epilogue_branch(gpio_end, 3)


def validate_returning_call_path(
    function: str,
    symbol_fragment: str,
    calls: tuple[str, ...],
    cleanup: tuple[tuple[str, str], ...] = (),
) -> None:
    reachable = reachable_instruction_prefix(function)
    call_indices = []
    search_from = 0
    for symbol in calls:
        found = next(
            (
                index
                for index in range(search_from, len(reachable))
                if any(symbol in target for target in direct_call_targets(reachable[index][4]))
            ),
            None,
        )
        if found is None:
            raise WitnessError(
                f"linked {symbol_fragment} path does not reach {symbol}"
            )
        call_indices.append(found)
        search_from = found + 1

    continuation = reachable[call_indices[-1]][0] + reachable[call_indices[-1]][1]
    instructions = {instruction[0]: instruction for instruction in parse_instructions(function)}
    for mnemonic, operands in cleanup:
        instruction = instructions.get(continuation)
        if instruction is None or not matches_instruction(instruction, mnemonic, operands):
            raise WitnessError(
                f"linked {symbol_fragment} path has a non-returning call continuation"
            )
        continuation += instruction[1]

    epilogue = instructions.get(continuation)
    if epilogue is None or not matches_instruction(
        epilogue, "pop", r"\s*\{\s*r7,\s*pc\s*\}\s*"
    ):
        raise WitnessError(
            f"linked {symbol_fragment} path has a non-returning call continuation"
        )


def validate_adapter_run_result(function: str) -> None:
    reachable = reachable_instruction_prefix(function)
    call = next(
        (
            instruction
            for instruction in reachable
            if any("board_sequence::run::<" in target for target in direct_call_targets(instruction[4]))
        ),
        None,
    )
    if call is None:
        raise WitnessError("linked board startup adapter result path does not reach the sequencer")

    start = call[0] + call[1]
    instructions = {instruction[0]: instruction for instruction in parse_instructions(function)}
    expected = (
        ("ldr", r"\s*r0,\s*\[sp,\s*#4\]\s*"),
        ("ldrb", r"\s*r0,\s*\[r0,\s*#6\]\s*"),
        ("lsls", r"\s*r0,\s*r0,\s*#31\s*"),
        ("cmp", r"\s*r0,\s*#0\s*"),
        ("bne.n", r"\s*[0-9a-fA-F]+\b.*"),
        ("b.n", r"\s*[0-9a-fA-F]+\b.*"),
        ("add", r"\s*r1,\s*sp,\s*#16\s*"),
        ("movs", r"\s*r0,\s*#0\s*"),
        ("strb", r"\s*r0,\s*\[r1,\s*#0\]\s*"),
        ("b.n", r"\s*[0-9a-fA-F]+\b.*"),
        ("add", r"\s*r1,\s*sp,\s*#16\s*"),
        ("movs", r"\s*r0,\s*#1\s*"),
        ("strb", r"\s*r0,\s*\[r1,\s*#0\]\s*"),
        ("b.n", r"\s*[0-9a-fA-F]+\b.*"),
        ("add", r"\s*r0,\s*sp,\s*#16\s*"),
        ("ldrb", r"\s*r0,\s*\[r0,\s*#0\]\s*"),
        ("add", r"\s*sp,\s*#32\s*"),
        ("pop", r"\s*\{\s*r7,\s*pc\s*\}\s*"),
    )
    path = [instructions.get(start + offset * 2) for offset in range(len(expected))]
    if not all(path) or any(
        not matches_instruction(instruction, mnemonic, operands)
        for instruction, (mnemonic, operands) in zip(path, expected)
    ):
        raise WitnessError("linked board startup adapter result path has an unexpected continuation")

    stopped_branch, skip_success, success_return, error_return = (
        path[4],
        path[5],
        path[9],
        path[13],
    )
    if not (
        branch_target(stopped_branch) == start + 20
        and branch_target(skip_success) == start + 12
        and branch_target(success_return) == start + 28
        and branch_target(error_return) == start + 28
    ):
        raise WitnessError("linked board startup adapter result paths do not reach the shared return")


def validate_sequencer_continuation(sequence: str) -> None:
    instructions = parse_instructions(sequence)
    by_address = {instruction[0]: instruction for instruction in instructions}
    calls = [
        instruction
        for instruction in reachable_instruction_prefix(sequence)
        if any("Backend>::call" in target for target in direct_call_targets(instruction[4]))
    ]
    if len(calls) < 7:
        raise WitnessError(
            "linked sequencer has no reachable ordered startup dispatch and continuation"
        )

    for variant, call in enumerate(calls[:7]):
        following = by_address.get(call[0] + call[1])
        operand = r"\s*r0,\s*\[sp,\s*#0\]\s*" if variant < 6 else r"\s*r0,\s*\[sp,\s*#4\]\s*"
        if following is None or not matches_instruction(following, "ldr", operand):
            raise WitnessError(
                f"linked sequencer variant {variant} has an unexpected call continuation"
            )

    tail = calls[6][0] + calls[6][1]
    guard = []
    for _ in range(5):
        instruction = by_address.get(tail)
        if instruction is None:
            break
        guard.append(instruction)
        tail += instruction[1]
    if len(guard) != 5 or not (
        matches_instruction(guard[0], "ldr", r"\s*r0,\s*\[sp,\s*#4\]\s*")
        and matches_instruction(guard[1], "ldrb", r"\s*r0,\s*\[r0,\s*#0\]\s*")
        and matches_instruction(guard[2], "lsls", r"\s*r0,\s*r0,\s*#31\s*")
        and matches_instruction(guard[3], "cmp", r"\s*r0,\s*#0\s*")
        and guard[4][2] == "bne.n"
    ):
        raise WitnessError("linked sequencer has an unexpected continuation after its first seven calls")

    variant_seven = branch_target(guard[4])
    skip_variant = by_address.get(tail)
    if variant_seven is None or skip_variant is None or skip_variant[2] != "b.n":
        raise WitnessError("linked sequencer variant-seven continuation has an unexpected branch")
    next_stage = branch_target(skip_variant)
    variant_seven_body = [
        by_address.get(variant_seven),
        by_address.get(variant_seven + 2),
    ]
    if not (
        all(variant_seven_body)
        and matches_instruction(variant_seven_body[0], "ldr", r"\s*r0,\s*\[sp,\s*#0\]\s*")
        and matches_instruction(variant_seven_body[1], "movs", r"\s*r1,\s*#7\s*")
    ):
        raise WitnessError("linked sequencer variant-seven dispatch has an unexpected continuation")
    variant_call = by_address.get(variant_seven + 4)
    if (
        variant_call is None
        or not any("Backend>::call" in target for target in direct_call_targets(variant_call[4]))
    ):
        raise WitnessError("linked sequencer variant-seven dispatch does not call its backend")
    variant_return = by_address.get(variant_call[0] + variant_call[1])
    if (
        next_stage is None
        or variant_return is None
        or variant_return[2] != "b.n"
        or branch_target(variant_return) != next_stage
    ):
        raise WitnessError("linked sequencer variant-seven call does not return to the next stage")

    if next_stage is None:
        raise WitnessError("linked sequencer optional calls have no continuation")

    def expect_optional(
        address: int, mnemonic: str, operands: str, name: str
    ) -> tuple[int, int, str, str, str]:
        instruction = by_address.get(address)
        if instruction is None or not matches_instruction(instruction, mnemonic, operands):
            raise WitnessError(f"linked sequencer optional {name} path has an unexpected continuation")
        return instruction

    expect_optional(next_stage, "add", r"\s*r0,\s*sp,\s*#8\s*", "CRC")
    expect_optional(next_stage + 2, "ldrb", r"\s*r0,\s*\[r0,\s*#1\]\s*", "CRC")
    expect_optional(next_stage + 4, "lsls", r"\s*r0,\s*r0,\s*#31\s*", "CRC")
    expect_optional(next_stage + 6, "cmp", r"\s*r0,\s*#0\s*", "CRC")
    crc_enabled_branch = expect_optional(
        next_stage + 8, "bne.n", r"\s*[0-9a-fA-F]+\b.*", "CRC"
    )
    skip_second_flag = expect_optional(
        next_stage + 10, "b.n", r"\s*[0-9a-fA-F]+\b.*", "CRC"
    )
    second_flag = branch_target(skip_second_flag)
    if second_flag is None or branch_target(crc_enabled_branch) is None:
        raise WitnessError("linked sequencer optional FM/CRC branches have no target")

    expect_optional(second_flag, "add", r"\s*r0,\s*sp,\s*#8\s*", "CRC")
    expect_optional(second_flag + 2, "ldrb", r"\s*r0,\s*\[r0,\s*#2\]\s*", "CRC")
    expect_optional(second_flag + 4, "lsls", r"\s*r0,\s*r0,\s*#31\s*", "CRC")
    expect_optional(second_flag + 6, "cmp", r"\s*r0,\s*#0\s*", "CRC")
    skip_crc = expect_optional(
        second_flag + 8, "beq.n", r"\s*[0-9a-fA-F]+\b.*", "CRC"
    )
    branch_to_crc = expect_optional(
        second_flag + 10, "b.n", r"\s*[0-9a-fA-F]+\b.*", "CRC"
    )
    crc_call = branch_target(crc_enabled_branch)
    if not (
        branch_target(skip_second_flag) == second_flag
        and branch_target(skip_crc) is not None
        and branch_target(branch_to_crc) == crc_call
        and crc_call is not None
    ):
        raise WitnessError("linked sequencer optional branches do not reach their next stage")

    expect_optional(crc_call, "ldr", r"\s*r0,\s*\[sp,\s*#0\]\s*", "CRC")
    expect_optional(crc_call + 2, "movs", r"\s*r1,\s*#8\s*", "CRC")
    expect_optional(
        crc_call + 4, "bl", r"\s*[0-9a-fA-F]+\s+<.*Backend>::call>.*", "CRC"
    )
    epilogue = branch_target(
        expect_optional(crc_call + 8, "b.n", r"\s*[0-9a-fA-F]+\b.*", "CRC")
    )
    if epilogue is None or branch_target(skip_crc) != epilogue:
        raise WitnessError("linked sequencer optional CRC paths do not reach the shared return epilogue")

    epilogue_instructions = [by_address.get(epilogue), by_address.get(epilogue + 2)]
    if not (
        all(epilogue_instructions)
        and matches_instruction(epilogue_instructions[0], "add", r"\s*sp,\s*#16\s*")
        and matches_instruction(epilogue_instructions[1], "pop", r"\s*\{\s*r7,\s*pc\s*\}\s*")
    ):
        raise WitnessError("linked sequencer has an unexpected shared return epilogue")


def validate_call_path(disassembly: str) -> None:
    for caller, callee in (
        ("Reset_Handler", "witness_main"),
        ("witness_main", "run_hardware"),
        ("run_hardware", "board_startup_adapters::run::<"),
        ("board_startup_adapters::run::<", "board_sequence::run::<"),
    ):
        require_call_edge(disassembly, caller, callee)

    validate_returning_call_path(
        function_block(disassembly, "run_hardware"),
        "run_hardware",
        ("board_startup_adapters::run::<",),
        (("add", r"\s*sp,\s*#16\s*"),),
    )
    validate_adapter_run_result(
        function_block(disassembly, "board_startup_adapters::run::<")
    )

    sequence = function_block(disassembly, "board_sequence::run")
    validate_sequencer_continuation(sequence)
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

    for method, calls, cleanup in (
        (
            "device_clock_gates",
            ("apply_device_clock_gates",),
            (),
        ),
        (
            "systick",
            (
                "apply_to_hardware",
                "systick_startup_adapter::InvalidReload>>::map_err",
            ),
            (),
        ),
        (
            "portcon",
            ("apply_portcon_startup_plan",),
            (),
        ),
        (
            "gpio",
            ("apply_gpio_startup_plan",),
            (("add", r"\s*sp,\s*#56(?:\s+[@;].*)?\s*"),),
        ),
    ):
        validate_returning_call_path(
            function_block(disassembly, f"StartupIo>::{method}", suffix=True),
            f"target {method}",
            calls,
            cleanup,
        )

    validate_backend_dispatch(disassembly)


def validate_systick_handler(disassembly: str, handler_address: int) -> None:
    entry = next(
        (
            instruction
            for instruction in parse_instructions(disassembly)
            if instruction[0] == (handler_address & ~1)
        ),
        None,
    )
    if entry is None or not matches_instruction(entry, "bx", r"\s*lr\s*"):
        raise WitnessError("SysTick_Handler must begin with bx lr at its linked symbol")


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
        target = symbols.get(symbol)
        if target is None:
            raise WitnessError(f"ELF does not retain exact {symbol} symbol")
        if vector[slot] != (target | 1):
            raise WitnessError(f"vector {slot} does not point to {symbol} in Thumb state")

    disassembly = run_tool("objdump", "-d", "-C", str(elf))
    reset = function_block(disassembly, "Reset_Handler")
    if "witness_main" not in reset:
        raise WitnessError("Reset_Handler does not retain the witness startup call path")
    validate_systick_handler(disassembly, symbols["SysTick_Handler"])
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
