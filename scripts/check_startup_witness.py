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


def matches_call_symbol(symbol: str, target: str) -> bool:
    if symbol.endswith("::<"):
        return symbol in target
    return (
        re.search(
            rf"{re.escape(symbol)}(?:\+0x[0-9a-fA-F]+|::<.*>)?(?:>|$)",
            target,
        )
        is not None
    )


def validate_required_call_targets(disassembly: str) -> None:
    required_symbols = (
        ("witness_main", "witness_main", False),
        ("run_hardware", "run_hardware", False),
        ("board_startup_adapters::run::<", "board_startup_adapters::run::<", False),
        ("board_sequence::run::<", "board_sequence::run::<", False),
        ("Backend>::call", "board_sequence::Backend>::call", False),
        ("StartupIo>::device_clock_gates", "StartupIo>::device_clock_gates", True),
        ("StartupIo>::systick", "StartupIo>::systick", True),
        ("StartupIo>::portcon", "StartupIo>::portcon", True),
        ("StartupIo>::gpio", "StartupIo>::gpio", True),
        (
            "StartupIo>::backlight_init_hardware",
            "StartupIo>::backlight_init_hardware",
            True,
        ),
        ("StartupIo>::adc_init", "StartupIo>::adc_init", True),
        ("StartupIo>::st7565_init", "StartupIo>::st7565_init", True),
        ("target_spi0::startup_mmio", "target_spi0::startup_mmio", False),
        (
            "target_spi0::startup_with_io::<",
            "target_spi0::startup_with_io::<",
            False,
        ),
        ("apply_device_clock_gates", "apply_device_clock_gates", False),
        (
            "systick_startup_adapter::apply_to_hardware",
            "systick_startup_adapter::apply_to_hardware",
            True,
        ),
        (
            "adc_startup_adapter::physical::target::apply_to_hardware",
            "adc_startup_adapter::physical::target::apply_to_hardware",
            False,
        ),
        (
            "adc_startup_adapter::physical::apply_with_words::<",
            "adc_startup_adapter::physical::apply_with_words::<",
            False,
        ),
        (
            "adc_startup_adapter::apply_with_io::<",
            "adc_startup_adapter::apply_with_io::<",
            False,
        ),
        (
            "systick_startup_adapter::InvalidReload>>::map_err",
            "systick_startup_adapter::InvalidReload>>::map_err",
            False,
        ),
        ("apply_portcon_startup_plan", "apply_portcon_startup_plan", False),
        ("apply_gpio_startup_plan", "apply_gpio_startup_plan", False),
        (
            "gpio_init_plan::GpioPlan>::for_features",
            "gpio_init_plan::GpioPlan>::for_features",
            False,
        ),
        (
            "initialize_backlight_hardware",
            "initialize_backlight_hardware",
            False,
        ),
        (
            "backlight_startup_executor::apply_with_io",
            "backlight_startup_executor::apply_with_io",
            False,
        ),
        (
            "BacklightInitPlan>::from_snapshot",
            "BacklightInitPlan>::from_snapshot",
            False,
        ),
        ("BacklightInitPlan>::writes", "BacklightInitPlan>::writes", False),
        (
            "BacklightWrite]>::iter",
            "backlight_plan::BacklightWrite]>::iter",
            False,
        ),
        (
            "IntoIterator>::into_iter",
            "backlight_plan::BacklightWrite>> as core::iter::traits::collect::IntoIterator>::into_iter",
            False,
        ),
        (
            "Iterator>::next",
            "backlight_plan::BacklightWrite>> as core::iter::traits::iterator::Iterator>::next",
            False,
        ),
        (
            "Iterator>::copied::<",
            "backlight_plan::BacklightWrite> as core::iter::traits::iterator::Iterator>::copied",
            False,
        ),
        (
            "RegisterIo>::read",
            "backlight_startup_executor::RegisterIo>::read",
            False,
        ),
        (
            "RegisterIo>::write",
            "backlight_startup_executor::RegisterIo>::write",
            False,
        ),
        (
            "RegisterAddress>::from_register",
            "backlight_startup_executor::RegisterAddress>::from_register",
            False,
        ),
        (
            "RegisterAddress>::address",
            "backlight_startup_executor::RegisterAddress>::address",
            False,
        ),
        ("__aeabi_memcpy4", "__aeabi_memcpy4", False),
    )

    def function_entry(fragment: str, suffix: bool) -> tuple[int, str]:
        matches = []
        for line in disassembly.splitlines():
            match = re.match(r"\s*([0-9a-fA-F]+)\s+<(.+)>:\s*$", line)
            if match is None:
                continue
            name = match.group(2)
            if name.endswith(fragment) if suffix else fragment in name:
                matches.append((int(match.group(1), 16), name))
        if len(matches) != 1:
            raise WitnessError(
                f"required call target {fragment} does not resolve uniquely to its symbol entry"
            )
        return matches[0]

    def matches_symbol(symbol: str, target: str) -> bool:
        return matches_call_symbol(symbol, target)

    callers = (
        ("Reset_Handler", False),
        ("witness_main", False),
        ("run_hardware", False),
        ("board_startup_adapters::run::<", False),
        ("board_sequence::run::<", False),
        ("board_sequence::Backend>::call", False),
        ("StartupIo>::device_clock_gates", True),
        ("StartupIo>::systick", True),
        ("StartupIo>::portcon", True),
        ("StartupIo>::gpio", True),
        ("StartupIo>::backlight_init_hardware", True),
        ("StartupIo>::adc_init", True),
        ("StartupIo>::st7565_init", True),
        ("target_spi0::startup_mmio", False),
        ("adc_startup_adapter::physical::target::apply_to_hardware", False),
        ("adc_startup_adapter::physical::apply_with_words::<", False),
        ("initialize_backlight_hardware", False),
        ("backlight_startup_executor::apply_with_io", False),
        ("BacklightInitPlan>::from_snapshot", False),
        ("BacklightInitPlan>::writes", False),
        ("backlight_startup_executor::RegisterIo>::read", False),
        ("backlight_startup_executor::RegisterIo>::write", False),
    )
    for caller, suffix in callers:
        function = function_block(disassembly, caller, suffix=suffix)
        for instruction in parse_instructions(function):
            if instruction[2] not in ("bl", "blx"):
                continue
            edge = re.search(
                r"\bbl(?:x)?\s+([0-9a-fA-F]+)\s+<(.+)>$", instruction[4]
            )
            if edge is None:
                continue
            address, target = int(edge.group(1), 16), edge.group(2)
            matching_entries = {
                function_entry(selector, suffix)
                for symbol, selector, suffix in required_symbols
                if matches_symbol(symbol, target)
            }
            if not matching_entries:
                allowed_helpers = (
                    (
                        (
                            "HardwareIo as startup_adapter_witness::board_startup_adapters::StartupIo>::placeholder",
                            False,
                            r"<?startup_adapter_witness::board_startup_adapters::HardwareIo as startup_adapter_witness::board_startup_adapters::StartupIo>::placeholder",
                        ),
                        (
                            "core::result::Result<(), startup_adapter_witness::board_startup_adapters::StartupError>>::is_err",
                            False,
                            r"<?core::result::Result<\(\), startup_adapter_witness::board_startup_adapters::StartupError>>::is_err",
                        ),
                    )
                    if caller == "board_sequence::Backend>::call"
                    else ()
                )
                matching_helpers = {
                    function_entry(helper, suffix)
                    for helper, suffix, identity in allowed_helpers
                    if re.fullmatch(identity, target)
                }
                if len(matching_helpers) == 1:
                    helper_entry = next(iter(matching_helpers))
                    if (
                        re.search(r"\+0x[0-9a-fA-F]+$", target)
                        or helper_entry[0] != address
                        or not (
                            helper_entry[1] == target
                            or helper_entry[1].endswith(target)
                        )
                    ):
                        raise WitnessError(
                            f"allowed helper call target {target} does not resolve to its symbol entry"
                        )
                    continue
                raise WitnessError(
                    f"required caller {caller} has an unexpected call target {target}"
                )

            entry = next(iter(matching_entries), None)
            if (
                re.search(r"\+0x[0-9a-fA-F]+$", target)
                or len(matching_entries) != 1
                or entry is None
                or entry[0] != address
                or not (entry[1] == target or entry[1].endswith(target))
            ):
                raise WitnessError(
                    f"required call target {target} does not resolve to its symbol entry"
                )


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


def reachable_control_flow_analysis(
    function: str, description: str
) -> tuple[
    list[tuple[int, int, str, str, str]],
    dict[int, set[int]],
    dict[int, set[int]],
]:
    instructions = parse_instructions(function)
    if not instructions:
        raise WitnessError(f"{description} has no reachable instructions")

    by_address = {instruction[0]: instruction for instruction in instructions}
    index_by_address = {
        instruction[0]: index for index, instruction in enumerate(instructions)
    }
    words = {
        int(match.group(1), 16): int(match.group(2), 16)
        for line in function.splitlines()
        if (
            match := re.match(
                r"\s*([0-9a-fA-F]+):\s+(?:[0-9a-fA-F]+\s+)+\.word\s+0x([0-9a-fA-F]+)\b",
                line,
            )
        )
    }

    def indirect_targets(index: int) -> list[int]:
        if index < 2:
            return []
        address, _, mnemonic, operands, _ = instructions[index]
        previous, lookup = instructions[index - 2 : index]
        immediate = re.fullmatch(
            r"\s*r0,\s*pc,\s*#(\d+)(?:\s*@.*)?\s*", previous[3]
        )
        if not (
            mnemonic == "mov"
            and re.fullmatch(r"\s*pc,\s*r0\s*", operands)
            and previous[2] == "add"
            and immediate
            and lookup[0] == previous[0] + previous[1]
            and lookup[2] == "ldr"
            and re.fullmatch(
                r"\s*r0,\s*\[r0,\s*r1\](?:\s*@.*)?\s*", lookup[3]
            )
            and address == lookup[0] + lookup[1]
        ):
            return []

        table = ((previous[0] + 4) & ~3) + int(immediate.group(1))
        targets = []
        while table in words:
            targets.append(words[table] & ~1)
            table += 4
        if not targets or any(target not in by_address for target in targets):
            return []
        return targets

    pending = [instructions[0][0]]
    reachable = set()
    predecessors: dict[int, set[int]] = {}
    indirect_dispatches = {}
    while pending:
        address = pending.pop()
        if address in reachable:
            continue
        instruction = by_address.get(address)
        if instruction is None:
            raise WitnessError(f"{description} has an unsupported control-flow target")
        reachable.add(address)
        _, size, mnemonic, operands, line = instruction
        fallthrough = address + size
        successors = []

        if mnemonic in ("bl", "blx"):
            if not direct_call_targets(line):
                raise WitnessError(f"{description} has an unsupported indirect call")
            successors.append(fallthrough)
        elif mnemonic in ("b", "b.n", "b.w"):
            target = branch_target(instruction)
            if target is None:
                raise WitnessError(f"{description} has an unsupported branch")
            successors.append(target)
        elif re.fullmatch(
            r"b(?:\.(?:n|w))?|b(?:eq|ne|cs|hs|cc|lo|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)(?:\.(?:n|w))?",
            mnemonic,
        ):
            target = branch_target(instruction)
            if target is None:
                raise WitnessError(f"{description} has an unsupported conditional branch")
            successors.extend((target, fallthrough))
        elif mnemonic == "bx":
            if not re.fullmatch(r"\s*lr\s*", operands):
                raise WitnessError(f"{description} has an unsupported indirect branch")
        elif mnemonic in ("udf", "bkpt", "svc"):
            raise WitnessError(f"{description} has an unsupported trap instruction")
        elif (mnemonic == "pop" or mnemonic.startswith("ldm")) and re.search(
            r"\bpc\b", operands
        ):
            pass
        elif mnemonic == "mov" and re.match(r"\s*pc\s*,", operands):
            targets = indirect_targets(index_by_address[address])
            if not targets:
                raise WitnessError(f"{description} has an unsupported indirect branch")
            lookup = instructions[index_by_address[address] - 1]
            setup = instructions[index_by_address[address] - 2]
            indirect_dispatches[address] = (setup[0], lookup[0])
            successors.extend(targets)
        elif is_control_transfer(mnemonic, operands, line):
            raise WitnessError(f"{description} has an unsupported control-flow instruction")
        else:
            successors.append(fallthrough)

        for successor in successors:
            if successor in by_address:
                predecessors.setdefault(successor, set()).add(address)
                pending.append(successor)
            elif successor != fallthrough:
                raise WitnessError(f"{description} has an unsupported control-flow target")

    for dispatch, (setup, lookup) in indirect_dispatches.items():
        if (
            predecessors.get(lookup) != {setup}
            or predecessors.get(dispatch) != {lookup}
        ):
            raise WitnessError(
                f"{description} has an indirect branch that bypasses its table setup"
            )

    entry = instructions[0][0]
    dominators = {address: set(reachable) for address in reachable}
    dominators[entry] = {entry}
    changed = True
    while changed:
        changed = False
        for address in sorted(reachable - {entry}):
            incoming = predecessors.get(address, set())
            common = set.intersection(*(dominators[pred] for pred in incoming))
            updated = {address} | common
            if updated != dominators[address]:
                dominators[address] = updated
                changed = True

    return (
        [instruction for instruction in instructions if instruction[0] in reachable],
        dominators,
        predecessors,
    )


def reachable_control_flow(
    function: str, description: str
) -> list[tuple[int, int, str, str, str]]:
    return reachable_control_flow_analysis(function, description)[0]


def indexed_dispatch_targets(
    function: str, description: str, count: int
) -> tuple[list[int], dict[int, tuple[int, int, str, str, str]]]:
    instructions = parse_instructions(function)
    prefix = reachable_instruction_prefix(function)
    if len(prefix) < 3 or not (
        matches_instruction(prefix[-3], "add", r"\s*r0,\s*pc,\s*#[0-9]+.*")
        and matches_instruction(prefix[-2], "ldr", r"\s*r0,\s*\[r0,\s*r1\].*")
        and matches_instruction(prefix[-1], "mov", r"\s*pc,\s*r0\s*")
    ):
        raise WitnessError(f"{description} has no reachable indexed dispatch from entry")

    table_match = re.search(
        r"\badr\s+r0,\s*([0-9a-fA-F]+)\b", prefix[-3][4]
    )
    if table_match is None:
        raise WitnessError(f"{description} has no reachable indexed dispatch table")
    table_address = int(table_match.group(1), 16)
    words = {
        int(match.group(1), 16): int(match.group(2), 16)
        for line in function.splitlines()
        if (
            match := re.match(
                r"\s*([0-9a-fA-F]+):\s+(?:[0-9a-fA-F]+\s+)+\.word\s+0x([0-9a-fA-F]+)\b",
                line,
            )
        )
    }
    try:
        targets = [
            words[table_address + offset] & ~1 for offset in range(0, count * 4, 4)
        ]
    except KeyError as error:
        raise WitnessError(f"{description} indexed dispatch table is incomplete") from error
    if table_address + count * 4 in words:
        raise WitnessError(f"{description} indexed dispatch table has unexpected cases")

    reachable, dominators, _ = reachable_control_flow_analysis(
        function, description
    )
    dispatch_address = prefix[-1][0]
    reachable_addresses = {instruction[0] for instruction in reachable}
    if any(
        target not in reachable_addresses
        or dispatch_address not in dominators[target]
        for target in targets
    ):
        raise WitnessError(f"{description} indexed dispatch does not reach every case")
    return targets, {instruction[0]: instruction for instruction in instructions}


def validate_register_mapping_input(function: str, description: str) -> None:
    prefix = reachable_instruction_prefix(function)
    expected = [
        ("push", r"\s*\{\s*r7,\s*lr\s*\}\s*", None),
        ("add", r"\s*r7,\s*sp,\s*#0\s*", None),
        ("sub", r"\s*sp,\s*#8\s*", None),
        ("uxtb", r"\s*r0,\s*r0\s*", None),
        ("str", r"\s*r0,\s*\[sp,\s*#0\]\s*", None),
        ("ldr", r"\s*r0,\s*\[sp,\s*#0\]\s*", None),
        ("lsls", r"\s*r1,\s*r0,\s*#2\s*", None),
        ("add", r"\s*r0,\s*pc,\s*#[0-9]+.*", None),
        ("ldr", r"\s*r0,\s*\[r0,\s*r1\].*", None),
        ("mov", r"\s*pc,\s*r0\s*", None),
    ]
    if not matches_instruction_sequence(prefix, expected):
        raise WitnessError(f"{description} has an unexpected indexed dispatch input")


def validate_backlight_plan_path(function: str, words: dict[int, int]) -> None:
    expected = """
push {r4, r5, r6, r7, lr}
add r7, sp, #12
sub sp, #196
movs r3, #23
lsls r3, r3, #17
orrs r1, r3
add r5, sp, #4
movs r4, #0
strb r4, [r5, #12]
str r4, [sp, #4]
str r3, [sp, #8]
str r1, [sp, #12]
add r1, sp, #20
movs r3, #1
strb r3, [r1, #12]
movs r1, #3
str r1, [sp, #20]
ldr r4, [pc, #168]
str r4, [sp, #28]
movs r4, #15
lsls r5, r4, #24
bics r2, r5
add r6, sp, #36
movs r4, #2
strb r4, [r6, #12]
str r3, [sp, #36]
str r5, [sp, #40]
str r2, [sp, #44]
ldr r2, [sp, #44]
lsls r3, r3, #24
orrs r2, r3
add r5, sp, #52
strb r4, [r5, #12]
str r4, [sp, #52]
str r3, [sp, #56]
str r2, [sp, #60]
add r2, sp, #68
strb r1, [r2, #12]
str r1, [sp, #68]
ldr r2, [pc, #128]
str r2, [sp, #76]
add r3, sp, #84
movs r2, #4
strb r2, [r3, #12]
str r1, [sp, #84]
movs r1, #5
str r1, [sp, #92]
""".strip().splitlines()
    for base in range(6):
        for offset in (12, 8, 4, 0):
            expected.extend(
                (f"ldr r1, [sp, #{4 + 16 * base + offset}]",
                 f"str r1, [sp, #{100 + 16 * base + offset}]")
            )
    expected.extend(("add r1, sp, #100", "movs r2, #96", "bl",
                     "add sp, #196", "pop {r4, r5, r6, r7, pc}"))
    path = reachable_instruction_prefix(function)
    if len(path) != len(expected):
        raise WitnessError("backlight plan operands or instruction continuation are incorrect")
    for instruction, wanted in zip(path, expected, strict=True):
        operands = re.split(r"\s+[@;]\s*", instruction[3], maxsplit=1)[0]
        actual = instruction[2] if wanted == "bl" else f"{instruction[2]} {operands}"
        if re.sub(r"\s+", "", actual) != re.sub(r"\s+", "", wanted):
            raise WitnessError("backlight plan operands or instruction continuation are incorrect")
        if wanted == "bl" and direct_call_targets(instruction[4]) != ["__aeabi_memcpy4"]:
            raise WitnessError("backlight plan operands or instruction continuation are incorrect")
        if wanted in ("ldr r4, [pc, #168]", "ldr r2, [pc, #128]"):
            immediate = 168 if wanted.startswith("ldr r4") else 128
            value = 0x3FF if immediate == 168 else 0x01010000
            if words.get(((instruction[0] + 4) & ~3) + immediate) != value:
                raise WitnessError("backlight plan assignments do not match the pinned register sequence")


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
    instruction: tuple[int, int, str, str, str] | None, mnemonic: str, operands: str
) -> bool:
    if instruction is None or instruction[2] != mnemonic:
        return False
    return any(
        re.fullmatch(operands, candidate) is not None
        for candidate in (
            instruction[3],
            re.split(r"\s+[@;]\s*", instruction[3], maxsplit=1)[0],
        )
    )


def matches_instruction_sequence(
    instructions: list[tuple[int, int, str, str, str]],
    expected: list[tuple[str, str, str | None]],
) -> bool:
    return len(instructions) == len(expected) and all(
        matches_instruction(instruction, mnemonic, operands)
        and (
            call_symbol is None
            or any(
                call_symbol in target
                for target in direct_call_targets(instruction[4])
            )
        )
        for instruction, (mnemonic, operands, call_symbol) in zip(
            instructions, expected, strict=True
        )
    )


def validate_exact_returning_instruction_path(
    function: str,
    description: str,
    *expected_sequences: list[tuple[str, str, str | None]],
) -> None:
    path = reachable_instruction_prefix(function)
    if not any(
        matches_instruction_sequence(path, expected)
        for expected in expected_sequences
    ):
        raise WitnessError(f"{description} has an unexpected instruction continuation")


def validate_backlight_initializer_path(function: str) -> None:
    validate_exact_returning_instruction_path(
        function,
        "backlight hardware initializer",
        [
            (
                "bl",
                r"\s*[0-9a-fA-F]+\s+<.*>.*",
                "backlight_startup_executor::apply_with_io",
            ),
            ("pop", r"\s*\{\s*r7,\s*pc\s*\}\s*", None),
        ],
        [
            ("push", r"\s*\{\s*r7,\s*lr\s*\}\s*", None),
            ("add", r"\s*r7,\s*sp,\s*#0\s*", None),
            ("sub", r"\s*sp,\s*#8\s*", None),
            ("add", r"\s*r0,\s*sp,\s*#4\s*", None),
            (
                "bl",
                r"\s*[0-9a-fA-F]+\s+<.*>.*",
                "backlight_startup_executor::apply_with_io",
            ),
            ("add", r"\s*sp,\s*#8\s*", None),
            ("pop", r"\s*\{\s*r7,\s*pc\s*\}\s*", None),
        ],
    )


def validate_backlight_register_io_path(function: str, method: str) -> None:
    if method == "read":
        compact = [
            ("mov", r"\s*r0,\s*r1\s*", None),
            (
                "bl",
                r"\s*[0-9a-fA-F]+\s+<.*>.*",
                "RegisterAddress>::from_register",
            ),
            (
                "bl",
                r"\s*[0-9a-fA-F]+\s+<.*>.*",
                "RegisterAddress>::address",
            ),
            ("ldr", r"\s*r0,\s*\[r0,\s*#0\]\s*", None),
            ("pop", r"\s*\{\s*r7,\s*pc\s*\}\s*", None),
        ]
        emitted = [
            ("push", r"\s*\{\s*r7,\s*lr\s*\}\s*", None),
            ("add", r"\s*r7,\s*sp,\s*#0\s*", None),
            *compact,
        ]
    elif method == "write":
        compact = [
            ("mov", r"\s*r0,\s*r1\s*", None),
            (
                "bl",
                r"\s*[0-9a-fA-F]+\s+<.*>.*",
                "RegisterAddress>::from_register",
            ),
            (
                "bl",
                r"\s*[0-9a-fA-F]+\s+<.*>.*",
                "RegisterAddress>::address",
            ),
            ("ldr", r"\s*r2,\s*\[sp,\s*#4\]\s*", None),
            ("str", r"\s*r2,\s*\[r0,\s*#0\]\s*", None),
            ("pop", r"\s*\{\s*r7,\s*pc\s*\}\s*", None),
        ]
        emitted = [
            ("push", r"\s*\{\s*r7,\s*lr\s*\}\s*", None),
            ("add", r"\s*r7,\s*sp,\s*#0\s*", None),
            ("sub", r"\s*sp,\s*#8\s*", None),
            ("str", r"\s*r2,\s*\[sp,\s*#4\]\s*", None),
            *compact[:-1],
            ("add", r"\s*sp,\s*#8\s*", None),
            compact[-1],
        ]
    else:
        raise ValueError(f"unsupported backlight register I/O method: {method}")

    validate_exact_returning_instruction_path(
        function, f"backlight register {method}", emitted
    )


def validate_stopped_guard(
    prefix: list[tuple[int, int, str, str, str]],
    branch: tuple[int, int, str, str, str],
    instructions: list[tuple[int, int, str, str, str]],
) -> bool:
    if len(prefix) < 4:
        return False
    load, shift, compare = prefix[-4:-1]
    if not (
        matches_instruction(load, "ldrb", r"\s*r0,\s*\[r0,\s*#8\]\s*")
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
    highest_variant: int = 5,
    variant_stack_offset: int = 8,
    index_stack_offset: int = 0,
    self_stack_offset: int = 4,
) -> bool:
    if len(prefix) < 5:
        return False
    load, extend, save, compare = prefix[-5:-1]
    if not (
        matches_instruction(
            load, "ldr", rf"\s*r0,\s*\[sp,\s*#{variant_stack_offset}\]\s*"
        )
        and matches_instruction(extend, "uxtb", r"\s*r0,\s*r0\s*")
        and matches_instruction(
            save, "str", rf"\s*r0,\s*\[sp,\s*#{index_stack_offset}\]\s*"
        )
        and matches_instruction(compare, "cmp", rf"\s*r0,\s*#{highest_variant}\s*")
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
        matches_instruction(
            handler[0], "ldr", rf"\s*r1,\s*\[sp,\s*#{variant_stack_offset}\]\s*"
        )
        and matches_instruction(
            handler[1], "ldr", rf"\s*r0,\s*\[sp,\s*#{self_stack_offset}\]\s*"
        )
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
    target_count = 7
    highest_variant = target_count - 1
    stack_size = 24
    self_stack_offset = 8
    variant_stack_offset = 12
    index_stack_offset = 4
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
        (2, "sub", rf"\s*sp,\s*#{stack_size}\s*"),
        (3, "str", rf"\s*r0,\s*\[sp,\s*#{self_stack_offset}\]\s*"),
        (4, "str", rf"\s*r1,\s*\[sp,\s*#{variant_stack_offset}\]\s*"),
        (5, "ldrb", r"\s*r0,\s*\[r0,\s*#8\]\s*"),
        (6, "lsls", r"\s*r0,\s*r0,\s*#31\s*"),
        (7, "cmp", r"\s*r0,\s*#0\s*"),
        (8, "bne.n", r"\s*[0-9a-fA-F]+\b.*"),
        (9, "b.n", r"\s*[0-9a-fA-F]+\b.*"),
        (10, "ldr", rf"\s*r0,\s*\[sp,\s*#{variant_stack_offset}\]\s*"),
        (11, "uxtb", r"\s*r0,\s*r0\s*"),
        (12, "str", rf"\s*r0,\s*\[sp,\s*#{index_stack_offset}\]\s*"),
        (13, "cmp", rf"\s*r0,\s*#{highest_variant}\s*"),
        (14, "bhi.n", r"\s*[0-9a-fA-F]+\b.*"),
        (15, "ldr", rf"\s*r0,\s*\[sp,\s*#{index_stack_offset}\]\s*"),
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
        or not validate_variant_guard(
            prefix[:15],
            variant_branch,
            instructions,
            highest_variant,
            variant_stack_offset,
            index_stack_offset,
            self_stack_offset,
        )
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
        return [
            words[table_address + offset] & ~1
            for offset in range(0, target_count * 4, 4)
        ]
    except KeyError as error:
        raise WitnessError(
            f"sequencer backend dispatch table is missing one of its first {target_count} targets"
        ) from error


def validate_st7565_executor_path(disassembly: str) -> None:
    methods = (
        (
            "StartupIo>::st7565_init",
            "target ST7565 init",
            ("target_spi0::startup_mmio",),
        ),
        (
            "target_spi0::startup_mmio",
            "target ST7565 MMIO entry",
            ("target_spi0::startup_with_io::<",),
        ),
    )
    for symbol, description, calls in methods:
        validate_returning_call_path(
            function_block(disassembly, symbol, suffix=symbol.startswith("StartupIo")),
            description,
            calls,
            (("add", r"\s*sp,\s*#8\s*"),),
        )


def validate_st7565_stage(disassembly: str, backend: str) -> None:
    validate_st7565_executor_path(disassembly)
    targets = backend_dispatch_targets(backend)
    if len(targets) != 7:
        raise WitnessError("display variant is missing from sequencer backend dispatch")
    validate_st7565_backend_case(backend, targets[6])


def validate_st7565_backend_case(backend: str, address: int) -> None:
    instructions = {instruction[0]: instruction for instruction in parse_instructions(backend)}

    def expect(
        instruction_address: int, mnemonic: str, operands: str
    ) -> tuple[int, int, str, str, str]:
        instruction = instructions.get(instruction_address)
        if instruction is None or not matches_instruction(instruction, mnemonic, operands):
            raise WitnessError(
                "display variant has an unexpected argument/result continuation"
            )
        return instruction

    def expect_call(instruction_address: int) -> tuple[int, int, str, str, str]:
        instruction = instructions.get(instruction_address)
        if (
            instruction is None
            or instruction[2] not in ("bl", "blx")
            or not any(
                matches_call_symbol("StartupIo>::st7565_init", target)
                for target in direct_call_targets(instruction[4])
            )
        ):
            raise WitnessError("display variant has no reachable ST7565 startup call")
        return instruction

    def advance(instruction: tuple[int, int, str, str, str]) -> int:
        return instruction[0] + instruction[1]

    cursor = address
    for mnemonic, operands in (
        ("ldr", r"\s*r3,\s*\[sp,\s*#8\]\s*"),
        ("ldr", r"\s*r0,\s*\[r3,\s*#0\]\s*"),
        ("ldr", r"\s*r1,\s*\[r3,\s*#12\]\s*"),
        ("ldr", r"\s*r2,\s*\[r3,\s*#16\]\s*"),
        ("ldr", r"\s*r3,\s*\[r3,\s*#20\]\s*"),
    ):
        cursor = advance(expect(cursor, mnemonic, operands))
    cursor = advance(expect_call(cursor))
    for mnemonic, operands in (
        ("mov", r"\s*r1,\s*r0\s*"),
        ("add", r"\s*r0,\s*sp,\s*#20\s*"),
        ("strb", r"\s*r1,\s*\[r0,\s*#0\]\s*"),
        ("ldrb", r"\s*r0,\s*\[r0,\s*#0\]\s*"),
        ("cmp", r"\s*r0,\s*#255\s*"),
    ):
        cursor = advance(expect(cursor, mnemonic, operands))

    error_branch = expect(cursor, "bne.n", r"\s*[0-9a-fA-F]+\b.*")
    error_path = branch_target(error_branch)
    success_branch = expect(advance(error_branch), "b.n", r"\s*[0-9a-fA-F]+\b.*")
    epilogue = branch_target(success_branch)
    if error_path is None or epilogue is None:
        raise WitnessError("display variant has an unexpected result branch")

    cursor = error_path
    for mnemonic, operands in (
        ("ldr", r"\s*r1,\s*\[sp,\s*#8\]\s*"),
        ("ldr", r"\s*r0,\s*\[sp,\s*#20\]\s*"),
        ("movs", r"\s*r2,\s*#1\s*"),
        ("strb", r"\s*r2,\s*\[r1,\s*#8\]\s*"),
        ("strb", r"\s*r0,\s*\[r1,\s*#24\]\s*"),
    ):
        cursor = advance(expect(cursor, mnemonic, operands))
    error_return = expect(cursor, "b.n", r"\s*[0-9a-fA-F]+\b.*")
    if branch_target(error_return) != epilogue:
        raise WitnessError("display variant result paths do not reach the shared return epilogue")

    if not (
        matches_instruction(
            instructions.get(epilogue), "add", r"\s*sp,\s*#24\s*"
        )
        and matches_instruction(
            instructions.get(epilogue + 2), "pop", r"\s*\{\s*r7,\s*pc\s*\}\s*"
        )
    ):
        raise WitnessError("display variant has an unexpected shared return epilogue")


def validate_backend_dispatch(disassembly: str) -> None:
    backend = function_block(disassembly, ">::call", suffix=True)
    targets = backend_dispatch_targets(backend)
    target_count = 7
    stack_size = 24
    self_stack_offset = 8
    portcon_offset = 4
    gpio_offset = 5
    adc_offset = 6
    if len(set(targets)) != target_count:
        raise WitnessError(
            f"sequencer backend first {target_count} variants do not have distinct dispatch targets"
        )

    instructions = parse_instructions(backend)
    by_address = {instruction[0]: instruction for instruction in instructions}
    methods = (
        "StartupIo>::device_clock_gates",
        "StartupIo>::systick",
        "StartupIo>::portcon",
        "StartupIo>::gpio",
        "StartupIo>::backlight_init_hardware",
        "StartupIo>::adc_init",
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
        and matches_instruction(
            epilogue_instructions[0], "add", rf"\s*sp,\s*#{stack_size}\s*"
        )
        and matches_instruction(epilogue_instructions[1], "pop", r"\s*\{\s*r7,\s*pc\s*\}\s*")
    ):
        raise WitnessError("sequencer backend shared return epilogue has an unexpected shape")

    clock_end = expect(
        targets[0], "ldr", rf"\s*r0,\s*\[sp,\s*#{self_stack_offset}\]\s*", 0
    )
    clock_end = expect(clock_end, "ldr", r"\s*r0,\s*\[r0,\s*#0\]\s*", 0)
    clock_end = expect_call(clock_end, methods[0], 0)
    expect_epilogue_branch(clock_end, 0)

    systick_end = expect(
        targets[1], "ldr", rf"\s*r0,\s*\[sp,\s*#{self_stack_offset}\]\s*", 1
    )
    systick_end = expect(systick_end, "ldr", r"\s*r0,\s*\[r0,\s*#0\]\s*", 1)
    systick_end = expect(
        systick_end,
        "ldr",
        r"\s*r1,\s*\[pc,\s*#\d+\](?:\s+[@;].*)?\s*",
        1,
    )
    systick_end = expect_call(systick_end, methods[1], 1)
    systick_end = expect(systick_end, "mov", r"\s*r1,\s*r0\s*", 1)
    systick_end = expect(
        systick_end,
        "add",
        r"\s*r0,\s*sp,\s*#16\s*",
        1,
    )
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
    error_end = expect(
        error_handler, "ldr", rf"\s*r1,\s*\[sp,\s*#{self_stack_offset}\]\s*", 1
    )
    error_end = expect(error_end, "movs", r"\s*r0,\s*#1\s*", 1)
    error_end = expect(error_end, "strb", r"\s*r0,\s*\[r1,\s*#8\]\s*", 1)
    error_end = expect(error_end, "movs", r"\s*r0,\s*#255\s*", 1)
    error_end = expect(error_end, "strb", r"\s*r0,\s*\[r1,\s*#24\]\s*", 1)
    expect_epilogue_branch(error_end, 1)

    portcon_end = expect(
        targets[2], "ldr", rf"\s*r1,\s*\[sp,\s*#{self_stack_offset}\]\s*", 2
    )
    portcon_end = expect(portcon_end, "ldr", r"\s*r0,\s*\[r1,\s*#0\]\s*", 2)
    portcon_end = expect(
        portcon_end, "ldrb", rf"\s*r1,\s*\[r1,\s*#{portcon_offset}\]\s*", 2
    )
    portcon_end = expect_call(portcon_end, methods[2], 2)
    expect_epilogue_branch(portcon_end, 2)

    gpio_end = expect(
        targets[3], "ldr", rf"\s*r1,\s*\[sp,\s*#{self_stack_offset}\]\s*", 3
    )
    gpio_end = expect(gpio_end, "ldr", r"\s*r0,\s*\[r1,\s*#0\]\s*", 3)
    gpio_end = expect(
        gpio_end, "ldrb", rf"\s*r1,\s*\[r1,\s*#{gpio_offset}\]\s*", 3
    )
    gpio_end = expect_call(gpio_end, methods[3], 3)
    expect_epilogue_branch(gpio_end, 3)

    backlight_end = expect(
        targets[4], "ldr", rf"\s*r0,\s*\[sp,\s*#{self_stack_offset}\]\s*", 4
    )
    backlight_end = expect(backlight_end, "ldr", r"\s*r0,\s*\[r0,\s*#0\]\s*", 4)
    backlight_end = expect_call(
        backlight_end, methods[4], 4
    )
    expect_epilogue_branch(backlight_end, 4)

    adc_end = expect(
        targets[5], "ldr", rf"\s*r1,\s*\[sp,\s*#{self_stack_offset}\]\s*", 5
    )
    adc_end = expect(adc_end, "ldr", r"\s*r0,\s*\[r1,\s*#0\]\s*", 5)
    adc_end = expect(adc_end, "ldrh", rf"\s*r1,\s*\[r1,\s*#{adc_offset}\]\s*", 5)
    adc_end = expect_call(adc_end, methods[5], 5)
    expect_epilogue_branch(adc_end, 5)
    validate_st7565_stage(disassembly, backend)

def validate_backlight_executor_paths(function: str) -> None:
    instructions = reachable_control_flow(function, "backlight executor")
    by_address = {instruction[0]: instruction for instruction in instructions}
    prefix = reachable_instruction_prefix(function)
    prefix_calls = [
        target
        for instruction in prefix
        for target in direct_call_targets(instruction[4])
    ]
    expected_plan_calls = (
        "BacklightInitPlan>::from_snapshot",
        "BacklightInitPlan>::writes",
        "IntoIterator>::into_iter",
    )
    plan_call_indexes = [
        next((i for i, target in enumerate(prefix_calls) if symbol in target), None)
        for symbol in expected_plan_calls
    ]
    if (
        any(index is None for index in plan_call_indexes)
        or plan_call_indexes != sorted(plan_call_indexes)
        or not prefix
        or prefix[-1][2] not in ("b.n", "b.w")
        or len(prefix_calls) != len(expected_plan_calls)
    ):
        raise WitnessError(
            "backlight executor does not reach the initialization plan before its write loop"
        )
    expected_prefix = [
        ("push", r"\s*\{\s*r7,\s*lr\s*\}\s*", None),
        ("add", r"\s*r7,\s*sp,\s*#0\s*", None),
        ("sub", r"\s*sp,\s*#160\s*", None),
        ("str", r"\s*r0,\s*\[sp,\s*#16\]\s*", None),
        ("add", r"\s*r0,\s*sp,\s*#24\s*", None),
        ("str", r"\s*r0,\s*\[sp,\s*#20\]\s*", None),
        ("movs", r"\s*r2,\s*#0\s*", None),
        ("mov", r"\s*r1,\s*r2\s*", None),
        ("bl", r"\s*[0-9a-fA-F]+\s+<.*>", expected_plan_calls[0]),
        ("ldr", r"\s*r0,\s*\[sp,\s*#20\]\s*", None),
        ("bl", r"\s*[0-9a-fA-F]+\s+<.*>", expected_plan_calls[1]),
        ("bl", r"\s*[0-9a-fA-F]+\s+<.*>", expected_plan_calls[2]),
        ("str", r"\s*r0,\s*\[sp,\s*#120\]\s*", None),
        ("str", r"\s*r1,\s*\[sp,\s*#124\]\s*", None),
        ("b.n", r"\s*[0-9a-fA-F]+\b.*", None),
    ]
    if not matches_instruction_sequence(prefix, expected_prefix):
        raise WitnessError("backlight executor has an unexpected plan input continuation")

    loop_head = branch_target(prefix[-1])
    if loop_head is None:
        raise WitnessError("backlight executor has no reachable write loop")

    def expect(
        address: int, mnemonic: str, operands: str, description: str
    ) -> tuple[int, int, str, str, str]:
        instruction = by_address.get(address)
        if not matches_instruction(instruction, mnemonic, operands):
            raise WitnessError(f"backlight executor has an unexpected {description} continuation")
        return instruction

    def expect_call(
        address: int, symbol: str, description: str
    ) -> tuple[int, int, str, str, str]:
        instruction = by_address.get(address)
        if (
            instruction is None
            or instruction[2] not in ("bl", "blx")
            or not any(symbol in target for target in direct_call_targets(instruction[4]))
        ):
            raise WitnessError(f"backlight executor does not reach {description}")
        return instruction

    head = expect(loop_head, "add", r"\s*r0,\s*sp,\s*#128\s*", "iterator loop head")
    next_address = head[0] + head[1]
    next_address = expect(
        next_address, "add", r"\s*r1,\s*sp,\s*#120\s*", "iterator setup"
    )[0] + 2
    iterator_call = expect_call(next_address, "Iterator>::next", "the plan iterator")
    cursor = iterator_call[0] + iterator_call[1]
    cursor = expect(
        cursor, "ldr", r"\s*r0,\s*\[sp,\s*#128\]\s*", "iterator result"
    )[0] + 2
    cursor = expect(cursor, "adds", r"\s*r0,\s*r0,\s*#1\s*", "iterator option check")[0] + 2
    cursor = expect(cursor, "cmp", r"\s*r0,\s*#0\s*", "iterator option check")[0] + 2
    option_branch = expect(
        cursor, "beq.n", r"\s*[0-9a-fA-F]+\b.*", "iterator completion branch"
    )
    exit_address = branch_target(option_branch)
    dispatch_branch = expect(
        option_branch[0] + option_branch[1],
        "b.n",
        r"\s*[0-9a-fA-F]+\b.*",
        "nonempty iterator branch",
    )
    dispatch_setup = branch_target(dispatch_branch)
    if exit_address is None or dispatch_setup != dispatch_branch[0] + dispatch_branch[1]:
        raise WitnessError("backlight executor loop branches do not reach its cases and return")

    exit_path = [
        by_address.get(exit_address),
        by_address.get(exit_address + 2),
    ]
    if not (
        all(exit_path)
        and matches_instruction(exit_path[0], "add", r"\s*sp,\s*#160\s*")
        and matches_instruction(exit_path[1], "pop", r"\s*\{\s*r7,\s*pc\s*\}\s*")
    ):
        raise WitnessError("backlight executor loop has an unexpected shared return epilogue")

    dispatches = [
        instruction
        for instruction in instructions
        if matches_instruction(instruction, "mov", r"\s*pc,\s*r0\s*")
    ]
    raw_instructions = parse_instructions(function)
    dispatch = dispatches[0] if len(dispatches) == 1 else None
    dispatch_index = (
        next(
            (
                index
                for index, instruction in enumerate(raw_instructions)
                if dispatch is not None and instruction[0] == dispatch[0]
            ),
            None,
        )
    )
    setup = (
        raw_instructions[dispatch_index - 2]
        if dispatch_index is not None and dispatch_index >= 2
        else None
    )
    lookup = (
        raw_instructions[dispatch_index - 1]
        if dispatch_index is not None and dispatch_index >= 1
        else None
    )
    table_match = (
        re.search(r"\badr\s+r0,\s*([0-9a-fA-F]+)\b", setup[4])
        if setup is not None
        else None
    )
    words = {
        int(match.group(1), 16): int(match.group(2), 16)
        for line in function.splitlines()
        if (
            match := re.match(
                r"\s*([0-9a-fA-F]+):\s+(?:[0-9a-fA-F]+\s+)+\.word\s+0x([0-9a-fA-F]+)\b",
                line,
            )
        )
    }
    if not (
        dispatch is not None
        and dispatch_index is not None
        and matches_instruction(setup, "add", r"\s*r0,\s*pc,\s*#[0-9]+.*")
        and setup[0] + setup[1] == lookup[0]
        and matches_instruction(lookup, "ldr", r"\s*r0,\s*\[r0,\s*r1\]\s*")
        and lookup[0] + lookup[1] == dispatch[0]
        and table_match is not None
        and setup[0] >= dispatch_setup
    ):
        raise WitnessError("backlight executor has no reachable indexed write-case dispatch")

    cursor = dispatch_setup
    for mnemonic, operands in (
        ("ldr", r"\s*r0,\s*\[sp,\s*#140\]\s*"),
        ("str", r"\s*r0,\s*\[sp,\s*#156\]\s*"),
        ("ldr", r"\s*r0,\s*\[sp,\s*#136\]\s*"),
        ("str", r"\s*r0,\s*\[sp,\s*#152\]\s*"),
        ("ldr", r"\s*r0,\s*\[sp,\s*#132\]\s*"),
        ("str", r"\s*r0,\s*\[sp,\s*#148\]\s*"),
        ("ldr", r"\s*r0,\s*\[sp,\s*#128\]\s*"),
        ("str", r"\s*r0,\s*\[sp,\s*#144\]\s*"),
        ("ldr", r"\s*r0,\s*\[sp,\s*#144\]\s*"),
        ("str", r"\s*r0,\s*\[sp,\s*#12\]\s*"),
        ("ldr", r"\s*r0,\s*\[sp,\s*#12\]\s*"),
        ("lsls", r"\s*r1,\s*r0,\s*#2\s*"),
    ):
        instruction = expect(cursor, mnemonic, operands, "iterator dispatch input")
        cursor = instruction[0] + instruction[1]
    if cursor != setup[0]:
        raise WitnessError("backlight executor has an unexpected iterator dispatch continuation")

    table_address = int(table_match.group(1), 16)
    try:
        case_targets = [
            words[table_address + offset] & ~1 for offset in range(0, 16, 4)
        ]
    except KeyError as error:
        raise WitnessError("backlight executor write-case table is incomplete") from error
    if (
        table_address + 16 in words
        or len(set(case_targets)) != 4
        or case_targets != sorted(case_targets)
        or any(target not in by_address for target in case_targets)
    ):
        raise WitnessError("backlight executor has unreachable indexed write cases")

    expected_case_calls = (
        ("RegisterIo>::read", "RegisterIo>::write"),
        ("RegisterIo>::read", "RegisterIo>::write"),
        ("RegisterIo>::read", "RegisterIo>::write"),
        ("RegisterIo>::write",),
    )
    expected_case_sequences = (
        [
            ("ldr", r"\s*r0,\s*\[sp,\s*#16\]\s*", None),
            ("ldr", r"\s*r1,\s*\[sp,\s*#148\]\s*", None),
            ("str", r"\s*r1,\s*\[sp,\s*#8\]\s*", None),
            ("ldr", r"\s*r1,\s*\[sp,\s*#156\]\s*", None),
            ("bl", r"\s*[0-9a-fA-F]+\s+<.*>.*", "RegisterIo>::read"),
            ("ldr", r"\s*r1,\s*\[sp,\s*#8\]\s*", None),
            ("mov", r"\s*r2,\s*r0\s*", None),
            ("ldr", r"\s*r0,\s*\[sp,\s*#16\]\s*", None),
            ("orrs", r"\s*r2,\s*r1\s*", None),
            ("ldr", r"\s*r1,\s*\[sp,\s*#156\]\s*", None),
            ("bl", r"\s*[0-9a-fA-F]+\s+<.*>.*", "RegisterIo>::write"),
            ("b.n", r"\s*[0-9a-fA-F]+\b.*", None),
        ],
        [
            ("ldr", r"\s*r0,\s*\[sp,\s*#16\]\s*", None),
            ("ldr", r"\s*r1,\s*\[sp,\s*#148\]\s*", None),
            ("str", r"\s*r1,\s*\[sp,\s*#4\]\s*", None),
            ("ldr", r"\s*r1,\s*\[sp,\s*#156\]\s*", None),
            ("bl", r"\s*[0-9a-fA-F]+\s+<.*>.*", "RegisterIo>::read"),
            ("ldr", r"\s*r1,\s*\[sp,\s*#4\]\s*", None),
            ("mov", r"\s*r2,\s*r0\s*", None),
            ("ldr", r"\s*r0,\s*\[sp,\s*#16\]\s*", None),
            ("bics", r"\s*r2,\s*r1\s*", None),
            ("ldr", r"\s*r1,\s*\[sp,\s*#156\]\s*", None),
            ("bl", r"\s*[0-9a-fA-F]+\s+<.*>.*", "RegisterIo>::write"),
            ("b.n", r"\s*[0-9a-fA-F]+\b.*", None),
        ],
        [
            ("ldr", r"\s*r0,\s*\[sp,\s*#16\]\s*", None),
            ("ldr", r"\s*r1,\s*\[sp,\s*#148\]\s*", None),
            ("str", r"\s*r1,\s*\[sp,\s*#0\]\s*", None),
            ("ldr", r"\s*r1,\s*\[sp,\s*#156\]\s*", None),
            ("bl", r"\s*[0-9a-fA-F]+\s+<.*>.*", "RegisterIo>::read"),
            ("ldr", r"\s*r1,\s*\[sp,\s*#0\]\s*", None),
            ("mov", r"\s*r2,\s*r0\s*", None),
            ("ldr", r"\s*r0,\s*\[sp,\s*#16\]\s*", None),
            ("orrs", r"\s*r2,\s*r1\s*", None),
            ("ldr", r"\s*r1,\s*\[sp,\s*#156\]\s*", None),
            ("bl", r"\s*[0-9a-fA-F]+\s+<.*>.*", "RegisterIo>::write"),
            ("b.n", r"\s*[0-9a-fA-F]+\b.*", None),
        ],
        [
            ("ldr", r"\s*r0,\s*\[sp,\s*#16\]\s*", None),
            ("ldr", r"\s*r2,\s*\[sp,\s*#152\]\s*", None),
            ("ldr", r"\s*r1,\s*\[sp,\s*#156\]\s*", None),
            ("bl", r"\s*[0-9a-fA-F]+\s+<.*>.*", "RegisterIo>::write"),
            ("b.n", r"\s*[0-9a-fA-F]+\b.*", None),
        ],
    )
    case_branches = []
    for variant, (target, expected_calls) in enumerate(
        zip(case_targets, expected_case_calls, strict=True)
    ):
        case_calls = []
        case_instructions = []
        address = target
        for _ in range(32):
            instruction = by_address.get(address)
            if instruction is None:
                raise WitnessError(
                    f"backlight executor write case {variant} has an unsupported continuation"
                )
            if instruction[2] in ("b.n", "b.w"):
                case_branches.append(instruction)
                if branch_target(instruction) != loop_head:
                    raise WitnessError(
                        f"backlight executor write case {variant} does not return to its loop"
                    )
                case_instructions.append(instruction)
                break
            if re.fullmatch(
                r"b(?:\.(?:n|w))?|b(?:eq|ne|cs|hs|cc|lo|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)(?:\.(?:n|w))?",
                instruction[2],
            ) or instruction[2] == "bx" or (
                instruction[2] == "pop" and re.search(r"\bpc\b", instruction[3])
            ):
                raise WitnessError(
                    f"backlight executor write case {variant} has an unsupported branch"
                )
            case_calls.extend(direct_call_targets(instruction[4]))
            case_instructions.append(instruction)
            address += instruction[1]
        else:
            raise WitnessError(
                f"backlight executor write case {variant} has no loop continuation"
            )
        if not matches_instruction_sequence(
            case_instructions, expected_case_sequences[variant]
        ):
            raise WitnessError(
                f"backlight executor write case {variant} has an unexpected register continuation"
            )
        observed_calls = [
            "RegisterIo>::read" if "RegisterIo>::read" in target else "RegisterIo>::write"
            for target in case_calls
            if "RegisterIo>::read" in target or "RegisterIo>::write" in target
        ]
        if observed_calls != list(expected_calls):
            raise WitnessError(
                f"backlight executor write case {variant} has an unexpected register continuation"
            )

    all_calls = [
        target
        for instruction in instructions
        for target in direct_call_targets(instruction[4])
    ]
    required_call_kinds = (
        "BacklightInitPlan>::from_snapshot",
        "BacklightInitPlan>::writes",
        "IntoIterator>::into_iter",
        "Iterator>::next",
    )
    if (
        len(all_calls) != 11
        or any(
            not any(
                fragment in target
                for fragment in (*required_call_kinds, "RegisterIo>::read", "RegisterIo>::write")
            )
            for target in all_calls
        )
    ):
        raise WitnessError("backlight executor has an unsupported call continuation")

    branch_instructions = [
        instruction
        for instruction in instructions
        if re.fullmatch(
            r"b(?:\.(?:n|w))?|b(?:eq|ne|cs|hs|cc|lo|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)(?:\.(?:n|w))?",
            instruction[2],
        )
    ]
    if (
        len(branch_instructions) != 7
        or sum(instruction[2] == "beq.n" for instruction in branch_instructions) != 1
        or sum(instruction[2] in ("b.n", "b.w") for instruction in branch_instructions) != 6
        or len(case_branches) != 4
        or any(
            branch_target(instruction) != loop_head
            for instruction in case_branches
        )
        or sum(
            instruction[2] == "mov"
            and re.fullmatch(r"\s*pc,\s*r0\s*", instruction[3]) is not None
            for instruction in instructions
        )
        != 1
    ):
        raise WitnessError("backlight executor has an unsupported loop or case branch")


def validate_backlight_register_evidence(disassembly: str) -> None:
    def word_entries(function: str) -> dict[int, int]:
        entries = {}
        for line in function.splitlines():
            match = re.match(
                r"\s*([0-9a-fA-F]+):\s+(?:[0-9a-fA-F]+\s+)+\.word\s+0x([0-9a-fA-F]+)\b",
                line,
            )
            if match:
                entries[int(match.group(1), 16)] = int(match.group(2), 16)
        return entries

    address_body = function_block(
        disassembly, "backlight_startup_executor::RegisterAddress>::address"
    )
    address_words = word_entries(address_body)

    address_literals = [
        int(match.group(1), 16)
        for line in address_body.splitlines()
        if (match := re.search(r"\.word\s+0x([0-9a-fA-F]+)\b", line))
        and 0x4000_0000 <= int(match.group(1), 16) < 0x5000_0000
    ]
    expected_addresses = {
        0x400B4008,
        0x400B401C,
        0x400B0008,
        0x400B4004,
        0x400B4000,
    }
    if len(address_literals) != 5 or set(address_literals) != expected_addresses:
        raise WitnessError("backlight register address evidence is missing or incorrect")

    targets, by_address = indexed_dispatch_targets(
        address_body, "backlight register address mapping", 5
    )
    validate_register_mapping_input(address_body, "backlight register address mapping")
    mapped_addresses = []
    first_store = by_address.get(targets[0] + 2)
    first_branch = by_address.get(targets[0] + 4)
    epilogue = (
        branch_target(first_branch)
        if matches_instruction(first_store, "str", r"\s*r0,\s*\[sp,\s*#4\]\s*")
        and matches_instruction(first_branch, "b.n", r"\s*[0-9a-fA-F]+\b.*")
        else None
    )
    for register, target in enumerate(targets):
        load = by_address.get(target)
        literal = (
            re.search(r"[@;]\s*\(?([0-9a-fA-F]+)\b", load[4])
            if load is not None
            and matches_instruction(load, "ldr", r"\s*r0,\s*\[pc,\s*#[0-9]+\].*")
            else None
        )
        if literal is None or int(literal.group(1), 16) not in address_words:
            raise WitnessError("backlight register address mapping is incomplete")
        mapped_addresses.append(address_words[int(literal.group(1), 16)])
        store = by_address.get(target + 2)
        continuation = by_address.get(target + 4)
        if not (
            epilogue is not None
            and matches_instruction(
                store, "str", r"\s*r0,\s*\[sp,\s*#4\]\s*"
            )
            and matches_instruction(
                continuation, "b.n", r"\s*[0-9a-fA-F]+\b.*"
            )
            and branch_target(continuation) == epilogue
        ):
            raise WitnessError(
                "backlight register address case does not reach its shared return epilogue"
            )
    address_return = [
        by_address.get(epilogue),
        by_address.get(epilogue + 2) if epilogue is not None else None,
        by_address.get(epilogue + 4) if epilogue is not None else None,
    ]
    if not (
        epilogue is not None
        and all(address_return)
        and matches_instruction(
            address_return[0], "ldr", r"\s*r0,\s*\[sp,\s*#4\]\s*"
        )
        and matches_instruction(address_return[1], "add", r"\s*sp,\s*#8\s*")
        and matches_instruction(
            address_return[2], "pop", r"\s*\{\s*r7,\s*pc\s*\}\s*"
        )
    ):
        raise WitnessError(
            "backlight register address mapping has an unexpected shared return epilogue"
        )
    if mapped_addresses != [
        0x400B4008,
        0x400B401C,
        0x400B0008,
        0x400B4004,
        0x400B4000,
    ]:
        raise WitnessError("backlight register addresses do not match their register variants")

    conversion = function_block(
        disassembly, "backlight_startup_executor::RegisterAddress>::from_register"
    )
    conversion_targets, conversion_instructions = indexed_dispatch_targets(
        conversion, "backlight register conversion", 5
    )
    validate_register_mapping_input(conversion, "backlight register conversion")
    conversion_epilogue = None
    for register, target in enumerate(conversion_targets):
        body = [
            conversion_instructions.get(target),
            conversion_instructions.get(target + 2),
            conversion_instructions.get(target + 4),
            conversion_instructions.get(target + 6),
        ]
        if not (
            all(body)
            and matches_instruction(body[0], "add", r"\s*r1,\s*sp,\s*#4\s*")
            and matches_instruction(body[1], "movs", rf"\s*r0,\s*#{register}\s*")
            and matches_instruction(body[2], "strb", r"\s*r0,\s*\[r1,\s*#0\]\s*")
            and body[3][2] in ("b.n", "b.w")
            and branch_target(body[3]) is not None
        ):
            raise WitnessError("backlight register conversion does not preserve register variants")
        continuation = conversion_instructions.get(target + 6)
        if not matches_instruction(
            continuation, "b.n", r"\s*[0-9a-fA-F]+\b.*"
        ):
            raise WitnessError(
                "backlight register conversion case does not reach its shared return epilogue"
            )
        target_epilogue = branch_target(continuation)
        if conversion_epilogue is None:
            conversion_epilogue = target_epilogue
        elif target_epilogue != conversion_epilogue:
            raise WitnessError(
                "backlight register conversion cases do not share their return epilogue"
            )
    conversion_return = [
        conversion_instructions.get(conversion_epilogue),
        conversion_instructions.get(conversion_epilogue + 2)
        if conversion_epilogue is not None
        else None,
        conversion_instructions.get(conversion_epilogue + 4)
        if conversion_epilogue is not None
        else None,
    ]
    if not (
        conversion_epilogue is not None
        and all(conversion_return)
        and matches_instruction(
            conversion_return[0], "ldr", r"\s*r0,\s*\[sp,\s*#4\]\s*"
        )
        and matches_instruction(
            conversion_return[1], "add", r"\s*sp,\s*#8\s*"
        )
        and matches_instruction(
            conversion_return[2], "pop", r"\s*\{\s*r7,\s*pc\s*\}\s*"
        )
    ):
        raise WitnessError(
            "backlight register conversion has an unexpected shared return epilogue"
        )

    validate_backlight_executor_paths(
        function_block(disassembly, "backlight_startup_executor::apply_with_io")
    )

    initialize = function_block(disassembly, "initialize_backlight_hardware")
    validate_backlight_initializer_path(initialize)

    plan = function_block(disassembly, "BacklightInitPlan>::from_snapshot")
    validate_backlight_plan_path(plan, word_entries(plan))

    apply = function_block(disassembly, "backlight_startup_executor::apply_with_io")
    instructions, apply_dominators, apply_predecessors = reachable_control_flow_analysis(
        apply, "backlight executor"
    )
    apply_calls = [
        (instruction, target)
        for instruction in instructions
        for target in direct_call_targets(instruction[4])
    ]
    plan_calls = [
        next(
            (instruction[0] for instruction, target in apply_calls if symbol in target),
            None,
        )
        for symbol in ("BacklightInitPlan>::from_snapshot", "BacklightInitPlan>::writes")
    ]
    if (
        any(address is None for address in plan_calls)
        or plan_calls[0] not in apply_dominators[plan_calls[1]]
    ):
        raise WitnessError("backlight executor does not apply the ordered initialization plan")
    writes = function_block(disassembly, "BacklightInitPlan>::writes")
    validate_exact_returning_instruction_path(
        writes,
        "backlight initialization writes",
        [
            ("push", r"\s*\{\s*r7,\s*lr\s*\}\s*", None),
            ("add", r"\s*r7,\s*sp,\s*#0\s*", None),
            ("movs", r"\s*r1,\s*#6\s*", None),
            ("bl", r"\s*[0-9a-fA-F]+\s+<.*BacklightWrite.*>::iter>", "BacklightWrite"),
            ("bl", r"\s*[0-9a-fA-F]+\s+<.*>", "Iterator>::copied"),
            ("pop", r"\s*\{\s*r7,\s*pc\s*\}\s*", None),
        ],
    )

    register_calls = [
        (instruction, target)
        for instruction in instructions
        for target in direct_call_targets(instruction[4])
        if "RegisterIo>::read" in target or "RegisterIo>::write" in target
    ]
    kinds = [
        "read" if "RegisterIo>::read" in target else "write"
        for _, target in register_calls
    ]
    if kinds != ["read", "write", "read", "write", "read", "write", "write"]:
        raise WitnessError("backlight executor is missing ordered fresh RMW register evidence")

    if any(
        plan_calls[1] not in apply_dominators[instruction[0]]
        for instruction, _ in register_calls
    ):
        raise WitnessError("backlight executor does not apply the ordered initialization plan")
    instruction_indexes = {instruction[0]: index for index, instruction in enumerate(instructions)}
    for pair, operation in zip(
        (register_calls[0:2], register_calls[2:4], register_calls[4:6]),
        ("orrs", "bics", "orrs"),
        strict=True,
    ):
        read_instruction, write_instruction = pair[0][0], pair[1][0]
        read_index = instruction_indexes[read_instruction[0]]
        write_index = instruction_indexes[write_instruction[0]]
        before_read = instructions[read_index - 1] if read_index else None
        between = instructions[read_index + 1 : write_index]
        expected = (
            ("mov", r"\s*r2,\s*r0\s*"),
            (operation, r"\s*r2,\s*r1\s*"),
            ("ldr", r"\s*r1,\s*\[sp,\s*#156\](?:\s*@.*)?\s*"),
        )
        expected_index = 0
        matched_operations = []
        for instruction in between:
            mnemonic, operands = expected[expected_index]
            if matches_instruction(instruction, mnemonic, operands):
                matched_operations.append(instruction)
                expected_index += 1
                if expected_index == len(expected):
                    break
        path = [before_read, read_instruction, *matched_operations, write_instruction]
        if not (
            before_read is not None
            and matches_instruction(
                before_read, "ldr", r"\s*r1,\s*\[sp,\s*#156\](?:\s*@.*)?\s*"
            )
            and expected_index == len(expected)
            and read_instruction[0] in apply_dominators[write_instruction[0]]
            and all(
                previous[0] in apply_dominators[current[0]]
                for previous, current in zip(path, path[1:])
            )
        ):
            raise WitnessError("backlight executor is missing ordered fresh RMW register evidence")

    return_instructions = [
        instruction
        for instruction in instructions
        if (
            instruction[2] == "bx"
            and re.fullmatch(r"\s*lr\s*", instruction[3])
        )
        or (
            (instruction[2] == "pop" or instruction[2].startswith("ldm"))
            and re.search(r"\bpc\b", instruction[3])
        )
    ]
    final_write = register_calls[-1][0][0]
    if not all(
        final_write in apply_dominators[instruction[0]]
        for instruction in return_instructions
    ):
        instruction_by_address = {instruction[0]: instruction for instruction in instructions}
        if len(return_instructions) != 1 or return_instructions[0][2] != "pop":
            raise WitnessError("backlight executor returns before the ordered MMIO writes")

        return_instruction = return_instructions[0]
        epilogue = instruction_by_address.get(return_instruction[0] - 2)
        exit_branches = [
            instruction
            for instruction in instructions
            if instruction[2].startswith("b")
            and branch_target(instruction) == (epilogue[0] if epilogue else None)
        ]
        if not (
            epilogue is not None
            and matches_instruction(epilogue, "add", r"\s*sp,\s*#\d+\s*")
            and len(exit_branches) == 1
            and exit_branches[0][2] == "beq.n"
            and apply_predecessors.get(epilogue[0]) == {exit_branches[0][0]}
        ):
            raise WitnessError("backlight executor returns before the ordered MMIO writes")

        exit_branch = exit_branches[0]
        comparison = instruction_by_address.get(exit_branch[0] - 2)
        increment = instruction_by_address.get(exit_branch[0] - 4)
        option = instruction_by_address.get(exit_branch[0] - 6)
        next_call = instruction_by_address.get(exit_branch[0] - 10)
        if not (
            comparison is not None
            and matches_instruction(comparison, "cmp", r"\s*r0,\s*#0\s*")
            and increment is not None
            and matches_instruction(increment, "adds", r"\s*r0,\s*r0,\s*#1\s*")
            and option is not None
            and matches_instruction(
                option,
                "ldr",
                r"\s*r0,\s*\[sp,\s*#\d+\](?:\s+[@;].*)?\s*",
            )
            and next_call is not None
            and next_call[0] + next_call[1] == option[0]
            and any(
                "Iterator>::next" in target
                for target in direct_call_targets(next_call[4])
            )
        ):
            raise WitnessError("backlight executor returns before the ordered MMIO writes")

        dispatches = [
            instruction
            for instruction in instructions
            if instruction[2] == "mov"
            and re.fullmatch(r"\s*pc,\s*r0\s*", instruction[3])
        ]
        write_sites = {
            instruction[0]
            for instruction, target in register_calls
            if "RegisterIo>::write" in target
        }
        by_following_address = {
            instruction[0] + instruction[1]: instruction for instruction in instructions
        }
        back_edges = [
            (instruction, by_following_address.get(instruction[0]))
            for instruction in instructions
            if instruction[2] in ("b", "b.n", "b.w")
            and (target := branch_target(instruction)) is not None
            and target < instruction[0]
        ]
        if (
            len(dispatches) != 1
            or len(back_edges) != len(write_sites)
            or {previous[0] for _, previous in back_edges if previous is not None}
            != write_sites
            or len({branch_target(branch) for branch, _ in back_edges}) != 1
            or any(
                previous is None
                or not any(
                    "RegisterIo>::write" in target
                    for target in direct_call_targets(previous[4])
                )
                or dispatches[0][0] not in apply_dominators[previous[0]]
                for _, previous in back_edges
            )
            or exit_branch[0] + exit_branch[1] not in apply_dominators[dispatches[0][0]]
        ):
            raise WitnessError("backlight executor returns before the ordered MMIO writes")

    for method in ("read", "write"):
        io_body = function_block(
            disassembly, f"backlight_startup_executor::RegisterIo>::{method}"
        )
        validate_backlight_register_io_path(io_body, method)


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
                if any(
                    matches_call_symbol(symbol, target)
                    for target in direct_call_targets(reachable[index][4])
                )
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
        epilogue, "pop", r"\s*\{\s*(?:r[0-7],\s*)*r7,\s*pc\s*\}\s*"
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
        ("ldr", r"\s*r0,\s*\[sp,\s*#44\]\s*"),
        ("add", r"\s*sp,\s*#52\s*"),
        ("pop", r"\s*\{\s*r4,\s*r5,\s*r6,\s*r7,\s*pc\s*\}\s*"),
    )
    path = [instructions.get(start + offset * 2) for offset in range(len(expected))]
    if not all(path) or any(
        not matches_instruction(instruction, mnemonic, operands)
        for instruction, (mnemonic, operands) in zip(path, expected)
    ):
        raise WitnessError(
            "linked board startup adapter result path has an unexpected continuation"
        )


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
        (("add", r"\s*sp,\s*#28\s*"),),
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
        ("backlight_init_hardware", "initialize_backlight_hardware"),
        (
            "adc_init",
            "adc_startup_adapter::physical::target::apply_to_hardware",
        ),
    ):
        method_body = function_block(disassembly, f"StartupIo>::{method}", suffix=True)
        if not any(adapter in target for target in reachable_call_targets(method_body)):
            raise WitnessError(
                f"target {method} has no reachable linked call instruction to {adapter}"
            )

    method_body = function_block(
        disassembly, "StartupIo>::st7565_init", suffix=True
    )
    if not any(
        "target_spi0::startup_mmio" in target
        for target in reachable_call_targets(method_body)
    ):
        raise WitnessError(
            "target st7565_init has no reachable linked call to target_spi0::startup_mmio"
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
        (
            "backlight_init_hardware",
            ("initialize_backlight_hardware",),
            (),
        ),
        (
            "adc_init",
            ("adc_startup_adapter::physical::target::apply_to_hardware",),
            (),
        ),
    ):
        validate_returning_call_path(
            function_block(disassembly, f"StartupIo>::{method}", suffix=True),
            f"target {method}",
            calls,
            cleanup,
        )

    validate_returning_call_path(
        function_block(
            disassembly, "adc_startup_adapter::physical::target::apply_to_hardware"
        ),
        "target ADC executor",
        ("adc_startup_adapter::physical::apply_with_words::<",),
        (("add", r"\s*sp,\s*#8\s*"),),
    )
    validate_returning_call_path(
        function_block(
            disassembly, "adc_startup_adapter::physical::apply_with_words::<"
        ),
        "ADC word adapter",
        ("adc_startup_adapter::apply_with_io::<",),
        (("add", r"\s*sp,\s*#8\s*"),),
    )

    validate_backend_dispatch(disassembly)
    validate_backlight_register_evidence(disassembly)
    validate_required_call_targets(disassembly)


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
        "initialize_backlight_hardware",
        "StartupIo>::st7565_init",
        "target_spi0::startup_mmio",
        "target_spi0::startup_with_io::<",
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
