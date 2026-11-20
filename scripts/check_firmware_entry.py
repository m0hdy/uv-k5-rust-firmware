#!/usr/bin/env python3
"""Inspect the selected Cortex-M0 firmware entry and its reachable boot stages."""

import argparse
import re
import subprocess
import sys
from pathlib import Path


class EntryError(ValueError):
    pass


FUNCTION = re.compile(r"^\s*[0-9a-fA-F]+\s+<(.+)>:\s*$", re.MULTILINE)
INSTRUCTION = re.compile(
    r"^\s*([0-9a-fA-F]+):\s+(?:(?:[0-9a-fA-F]{4,8})\s+)*"
    r"([A-Za-z][A-Za-z0-9.]*)\s*(.*?)$",
    re.MULTILINE,
)
CONDITIONAL_BRANCH = re.compile(
    r"b(?:eq|ne|cs|cc|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)(?:\.n|\.w)?"
)
EEPROM_SUCCESS_TAG = 2
PREDICATE_PRESERVING_INSTRUCTIONS = {"nop", "str", "strb", "strh"}


def function_body(disassembly: str, symbol: str) -> str:
    functions = list(FUNCTION.finditer(disassembly))
    for index, match in enumerate(functions):
        if match.group(1) == symbol:
            end = functions[index + 1].start() if index + 1 < len(functions) else len(disassembly)
            return disassembly[match.end() : end]
    raise EntryError(f"disassembly is missing {symbol}")


def function_end_address(disassembly: str, symbol: str) -> int:
    functions = list(FUNCTION.finditer(disassembly))
    for index, match in enumerate(functions):
        if match.group(1) == symbol:
            if index + 1 == len(functions):
                raise EntryError(f"disassembly is missing the end of {symbol}")
            following = re.match(
                r"\s*([0-9a-fA-F]+)", disassembly[functions[index + 1].start() :]
            )
            if following:
                return int(following.group(1), 16)
    raise EntryError(f"disassembly is missing {symbol}")


def function_bodies(disassembly: str) -> dict[str, str]:
    functions = list(FUNCTION.finditer(disassembly))
    return {
        match.group(1): disassembly[
            match.end() : functions[index + 1].start()
            if index + 1 < len(functions)
            else len(disassembly)
        ]
        for index, match in enumerate(functions)
    }


def instructions(body: str) -> list[tuple[int, str, str]]:
    return [
        (int(match.group(1), 16), match.group(2).lower(), match.group(3))
        for match in INSTRUCTION.finditer(body)
    ]


def branch_target(operands: str) -> int | None:
    target = re.match(r"\s*([0-9a-fA-F]+)\b", operands)
    return int(target.group(1), 16) if target else None


def instruction_successors(
    rows: list[tuple[int, str, str]], index: int
) -> list[int]:
    _, mnemonic, operands = rows[index]
    following = rows[index + 1][0] if index + 1 < len(rows) else None
    target = branch_target(operands)
    if mnemonic in {"b", "b.n", "b.w"}:
        return [target] if target is not None else []
    if CONDITIONAL_BRANCH.fullmatch(mnemonic) or mnemonic in {"cbz", "cbnz"}:
        return [address for address in (target, following) if address is not None]
    if mnemonic.startswith("bx") or (
        mnemonic.startswith("pop") and re.search(r"\bpc\b", operands)
    ) or mnemonic.startswith("udf"):
        return []
    return [following] if following is not None else []


def reachable_instruction_addresses(
    rows: list[tuple[int, str, str]], starts: list[int] | None = None
) -> set[int]:
    if not rows:
        return set()
    indices = {address: index for index, (address, _, _) in enumerate(rows)}
    pending = list(starts or [rows[0][0]])
    reached = set()
    while pending:
        address = pending.pop()
        if address in reached or address not in indices:
            continue
        reached.add(address)
        pending.extend(instruction_successors(rows, indices[address]))
    return reached


def call_targets(body: str) -> list[str]:
    targets = []
    rows = instructions(body)
    reached = reachable_instruction_addresses(rows)
    for address, mnemonic, operands in rows:
        if address not in reached:
            continue
        if mnemonic.startswith("bl"):
            target = re.search(r"<(.+)>", operands)
            if target:
                targets.append(target.group(1))
    return targets


def require_call(body: str, target: str, minimum: int = 1) -> None:
    if sum(target in called for called in call_targets(body)) < minimum:
        raise EntryError(f"entry path does not call {target}")


def reachable_functions(functions: dict[str, str], start: str) -> set[str]:
    if start not in functions:
        raise EntryError(f"disassembly is missing {start}")
    reached = set()
    pending = [start]
    while pending:
        current = pending.pop()
        if current in reached:
            continue
        reached.add(current)
        for target in call_targets(functions[current]):
            if target in functions and target not in reached:
                pending.append(target)
    return reached


def matching(reached: set[str], fragment: str) -> list[str]:
    return [name for name in reached if fragment in name]


def require_reachable(reached: set[str], fragment: str) -> str:
    matches = matching(reached, fragment)
    if not matches:
        raise EntryError(f"selected entry cannot reach {fragment}")
    return matches[0]


def has_store(functions: dict[str, str], reached: set[str], start: str) -> bool:
    pending = [start]
    visited = set()
    while pending:
        current = pending.pop()
        if current in visited:
            continue
        visited.add(current)
        rows = instructions(functions[current])
        live = reachable_instruction_addresses(rows)
        if any(
            address in live and mnemonic.startswith("str")
            for address, mnemonic, _ in rows
        ):
            return True
        pending.extend(
            target
            for target in call_targets(functions[current])
            if target in reached and target not in visited
        )
    return False


def has_nontrivial_instruction(body: str) -> bool:
    rows = instructions(body)
    live = reachable_instruction_addresses(rows)
    return any(
        address in live
        and mnemonic not in {"bx", "pop", "push", "nop", "mov", "movs"}
        and not mnemonic.startswith("b")
        and not mnemonic.startswith("udf")
        for address, mnemonic, _ in rows
    )


def preserves_result_address(mnemonic: str, operands: str) -> bool:
    if mnemonic not in {"add", "ldr", "mov"}:
        return False
    destination = re.match(r"\s*(r[0-7]|sp)\s*,", operands)
    return destination is not None and destination.group(1) not in {"r0", "sp"}


def result_slot_before_call(
    rows: list[tuple[int, str, str]], call_index: int
) -> int:
    live = reachable_instruction_addresses(rows)
    indices = {address: index for index, (address, _, _) in enumerate(rows)}
    predecessors = {address: set() for address in live}
    for address in live:
        for successor in instruction_successors(rows, indices[address]):
            if successor in live:
                predecessors[successor].add(address)

    entry = rows[0][0]
    dominators = {
        address: {address} if address == entry else set(live)
        for address in live
    }
    changed = True
    while changed:
        changed = False
        for address in live - {entry}:
            incoming = predecessors[address]
            updated = {address} | (
                set.intersection(*(dominators[item] for item in incoming))
                if incoming
                else set()
            )
            if updated != dominators[address]:
                dominators[address] = updated
                changed = True

    call_address = rows[call_index][0]
    ancestors = set()
    pending = [call_address]
    while pending:
        address = pending.pop()
        if address in ancestors:
            continue
        ancestors.add(address)
        pending.extend(predecessors[address])

    valid = []
    for address in dominators[call_address]:
        _, mnemonic, operands = rows[indices[address]]
        definition = re.fullmatch(
            r"r0,\s*sp,\s*#(\d+)", operands.split("@")[0].strip()
        )
        if mnemonic != "add" or definition is None:
            continue
        between = (
            reachable_instruction_addresses(rows, [address]) & ancestors
        ) - {address, call_address}
        if all(
            preserves_result_address(row_mnemonic, row_operands)
            for item in between
            for _, row_mnemonic, row_operands in [rows[indices[item]]]
        ):
            valid.append(int(definition.group(1)))
    if len(valid) != 1:
        raise EntryError(
            "EEPROM result address lacks one dominating stack-relative definition"
        )
    return valid[0]


def vector_words(vector_dump: str) -> list[int]:
    words = []
    for line in vector_dump.splitlines():
        parts = line.split()
        if not parts or not re.fullmatch(r"[0-9a-fA-F]+", parts[0]):
            continue
        for part in parts[1:]:
            if re.fullmatch(r"[0-9a-fA-F]{8}", part):
                words.append(int.from_bytes(bytes.fromhex(part), "little"))
            else:
                break
    return words


def function_address(disassembly: str, symbol: str) -> int:
    for match in FUNCTION.finditer(disassembly):
        if match.group(1) == symbol:
            address = re.match(r"\s*([0-9a-fA-F]+)", disassembly[match.start() :])
            if address:
                return int(address.group(1), 16)
    raise EntryError(f"disassembly is missing {symbol}")


def require_result_guards(
    body: str,
    called_fragment: str,
    expected_calls: int,
    function_start: int,
    function_end: int,
) -> None:
    rows = instructions(body)
    live = reachable_instruction_addresses(rows)
    indices = {address: index for index, (address, _, _) in enumerate(rows)}
    calls = [
        index
        for index, (address, mnemonic, operands) in enumerate(rows)
        if address in live
        and mnemonic.startswith("bl")
        and (target := re.search(r"<(.+)>", operands))
        and called_fragment in target.group(1)
        and "+0x" not in target.group(1)
    ]
    if len(calls) != expected_calls:
        raise EntryError(f"selected coordinator must call {called_fragment} exactly {expected_calls} times")
    for index in calls:
        result_slot = result_slot_before_call(rows, index)
        following = [
            row for row in rows[index + 1 : index + 9] if row[0] in live
        ]
        conditional = next(
            (
                offset
                for offset, (_, mnemonic, _) in enumerate(following)
                if CONDITIONAL_BRANCH.fullmatch(mnemonic)
            ),
            None,
        )
        if conditional is None:
            raise EntryError(f"{called_fragment} result is not conditionally checked before later effects")
        comparisons = [
            (offset, re.fullmatch(r"(r[0-7]),\s*#(\d+)", operands.strip()))
            for offset, (_, mnemonic, operands) in enumerate(following[:conditional])
            if mnemonic == "cmp"
        ]
        if len(comparisons) != 1 or comparisons[0][1] is None:
            raise EntryError(
                f"{called_fragment} payload result predicate is not a single comparison"
            )
        comparison, comparison_match = comparisons[0]
        compared_register = comparison_match.group(1)
        if int(comparison_match.group(2)) != EEPROM_SUCCESS_TAG:
            raise EntryError(
                f"{called_fragment} payload result tag does not use the success code"
            )
        before_comparison = following[:comparison]
        if len(before_comparison) != 1:
            raise EntryError(
                f"{called_fragment} result payload has unknown control or operand changes"
            )
        _, payload_mnemonic, payload_operands = before_comparison[0]
        payload_load = re.fullmatch(
            rf"{compared_register},\s*\[sp,\s*#(\d+)\]",
            payload_operands.split("@")[0].strip(),
        )
        if payload_mnemonic != "ldr" or payload_load is None:
            raise EntryError(
                f"{called_fragment} comparison is not loaded from its result payload"
            )
        payload_slot = int(payload_load.group(1))
        if payload_slot != result_slot:
            raise EntryError(
                f"{called_fragment} comparison does not reload its returned result slot"
            )
        if any(
            mnemonic not in PREDICATE_PRESERVING_INSTRUCTIONS
            for _, mnemonic, _ in following[comparison + 1 : conditional]
        ):
            raise EntryError(
                f"{called_fragment} payload result predicate flags are overwritten"
            )
        branch_address = following[conditional][0]
        branch_mnemonic = following[conditional][1]
        if not re.fullmatch(r"b(?:eq|ne)(?:\.n|\.w)?", branch_mnemonic):
            raise EntryError(
                f"{called_fragment} payload result tag is not checked for equality"
            )
        target = branch_target(following[conditional][2])
        if target is None or not function_start <= target < function_end:
            raise EntryError(
                f"{called_fragment} failure branch does not remain in the coordinator"
            )

        call_number = calls.index(index)
        if call_number + 1 < len(calls):
            continuation = rows[calls[call_number + 1]][0]
        else:
            continuation = next(
                (
                    address
                    for address, mnemonic, operands in rows[index + 1 :]
                    if address in live
                    and mnemonic.startswith("bl")
                    and "adc_samples::decode_adc_sample" in operands
                ),
                None,
            )
        if continuation is None:
            raise EntryError(
                f"{called_fragment} result has no reachable later startup stage"
            )
        successors = instruction_successors(rows, indices[branch_address])
        if len(successors) != 2:
            raise EntryError(
                f"{called_fragment} result does not split continuation from first-error return"
            )
        equality_successor = (
            successors[0] if branch_mnemonic.startswith("beq") else successors[1]
        )
        error_successor = (
            successors[1] if branch_mnemonic.startswith("beq") else successors[0]
        )
        success_path = reachable_instruction_addresses(rows, [equality_successor])
        error_path = reachable_instruction_addresses(rows, [error_successor])
        if continuation not in success_path or continuation in error_path:
            raise EntryError(
                f"{called_fragment} success tag does not select continuation"
            )
        short_circuit = error_successor
        short_path = reachable_instruction_addresses(rows, [short_circuit])
        returns = {
            address
            for address, mnemonic, operands in rows
            if mnemonic.startswith("bx")
            or (mnemonic.startswith("pop") and re.search(r"\bpc\b", operands))
        }
        if not short_path & returns:
            raise EntryError(
                f"{called_fragment} first-error path does not return from the coordinator"
            )


def reject_entry_self_loops(disassembly: str, functions: dict[str, str]) -> None:
    for symbol in ("firmware_main", "firmware_integration_entry"):
        address = function_address(disassembly, symbol)
        for _, mnemonic, operands in instructions(functions[symbol]):
            if mnemonic in {"b", "b.n", "b.w"}:
                target = re.match(r"\s*([0-9a-fA-F]+)\b", operands)
                if target and int(target.group(1), 16) == address:
                    raise EntryError(f"{symbol} contains an entry self-loop")


def inspect_disassembly(disassembly: str, symbols: str, vector_dump: str) -> None:
    functions = function_bodies(disassembly)
    reset = functions.get("Reset_Handler")
    main = functions.get("firmware_main")
    entry = functions.get("firmware_integration_entry")
    fail_closed = functions.get("firmware_fail_closed")
    systick = functions.get("SysTick_Handler")
    if any(value is None for value in (reset, main, entry, fail_closed, systick)):
        raise EntryError("disassembly is missing a required entry or handler")

    reset_instructions = instructions(reset)
    if not reset_instructions or reset_instructions[0][1:3] != ("cpsid", "i"):
        raise EntryError("Reset_Handler must mask interrupts before its first effect")
    reject_entry_self_loops(disassembly, functions)
    require_call(reset, "firmware_main")
    require_call(main, "firmware_integration_entry")
    require_call(entry, "coordinate_with_welcome::<")
    require_call(entry, "firmware_fail_closed")
    if not re.search(r"\bbx\s+lr\b", systick):
        raise EntryError("selected vector table's SysTick_Handler is not a valid return handler")
    if not re.search(r"\bcpsid\s+i\b", fail_closed) or not re.search(r"\bwfi\b", fail_closed):
        raise EntryError("firmware_fail_closed must mask interrupts and park")

    words = vector_words(vector_dump)
    if len(words) != 48:
        raise EntryError("selected vector table must contain exactly 48 words")
    reset_vector = words[1]
    if words[0] != 0x20003FF0:
        raise EntryError("selected vector table has an invalid initial stack pointer")
    if reset_vector & ~1 != function_address(disassembly, "Reset_Handler") or reset_vector & 1 == 0:
        raise EntryError("selected reset vector does not target Reset_Handler in Thumb state")
    if words[15] & ~1 != function_address(disassembly, "SysTick_Handler") or words[15] & 1 == 0:
        raise EntryError("selected SysTick vector does not target SysTick_Handler in Thumb state")

    selected = reachable_functions(functions, "Reset_Handler")
    if not any("firmware_integration_entry" in name for name in selected):
        raise EntryError("Reset-to-firmware_main path bypasses firmware_integration_entry")
    coordinator = require_reachable(selected, "coordinate_with_welcome::<")
    startup_path = reachable_functions(functions, coordinator)
    coordinator_address = function_address(disassembly, coordinator)
    require_result_guards(
        functions[coordinator],
        "eeprom_read_transport::execute_read::<",
        3,
        coordinator_address,
        function_end_address(disassembly, coordinator),
    )
    for stage in (
        "StartupIo>::systick",
        "StartupIo>::portcon",
        "StartupIo>::gpio",
        "StartupIo>::backlight_init_hardware",
        "StartupIo>::adc_init",
        "StartupIo>::st7565_init",
    ):
        executor = require_reachable(startup_path, stage)
        if not has_store(functions, startup_path, executor):
            raise EntryError(f"selected {stage} executor has no reachable hardware effect")
    reads = sum(
        "eeprom_read_transport::execute_read::<" in target and target in functions
        for name in startup_path
        for target in call_targets(functions[name])
    )
    if reads != 3:
        raise EntryError("firmware entry must execute the three fallible EEPROM settings reads")
    for executor in (
        "adc_samples::decode_adc_sample",
        "adc_samples::calibrated_battery_voltage",
        "startup_version_display::startup_version_display_plan",
        "startup_version_row_plan::compose_startup_version_row",
    ):
        name = require_reachable(startup_path, executor)
        if not has_nontrivial_instruction(functions[name]):
            raise EntryError(f"selected {executor} executor is a no-op")
    display_initializers = sum(
        "StartupIo>::st7565_init" in target and target in functions
        for name in startup_path
        for target in call_targets(functions[name])
    )
    if display_initializers != 1:
        raise EntryError("firmware entry must initialize the board display exactly once")
    if "GpioI2cIo<" not in symbols or "systick_delay_adapter::SysTickDelay<" not in symbols:
        raise EntryError("selected entry lacks the concrete GPIO I2C/SysTick delay backend")
    delay_functions = [
        name
        for name in functions
        if "SysTickDelay<" in name and ("::delay_ms" in name or "::delay_1us" in name)
    ]
    if not delay_functions or any(
        not has_nontrivial_instruction(functions[name])
        for name in delay_functions
    ):
        raise EntryError("selected entry lacks emitted counter-polling delay providers")
    entry_calls = call_targets(entry)
    coordinate_indices = [
        index for index, target in enumerate(entry_calls) if "coordinate_with_welcome::<" in target
    ]
    if not coordinate_indices or any(
        "firmware_fail_closed" not in target for target in entry_calls[coordinate_indices[0] + 1 :]
    ):
        raise EntryError("startup result must terminate fail-closed without later entry effects")
    if re.search(r"\bcpsie\s+i\b", disassembly):
        raise EntryError("selected startup must not unmask interrupts without installed handlers")
    if "0xe000e180" not in disassembly or "0xe000e100" in disassembly:
        raise EntryError("SARADC must remain masked without an NVIC enable write")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--elf", required=True, type=Path)
    parser.add_argument(
        "--objdump",
        default=f"{__import__('os').environ.get('ARM_BINUTILS_PREFIX', 'arm-none-eabi-')}objdump",
    )
    args = parser.parse_args()

    try:
        disassembly = subprocess.run(
            [args.objdump, "-d", "-C", "--wide", str(args.elf)],
            check=True,
            capture_output=True,
            text=True,
        ).stdout
        vector_dump = subprocess.run(
            [args.objdump, "-s", "-j", ".vector_table", str(args.elf)],
            check=True,
            capture_output=True,
            text=True,
        ).stdout
        symbols = subprocess.run(
            [args.objdump, "-t", "-C", "--wide", str(args.elf)],
            check=True,
            capture_output=True,
            text=True,
        ).stdout
        inspect_disassembly(disassembly, symbols, vector_dump)
    except (OSError, subprocess.CalledProcessError, EntryError) as error:
        print(f"firmware entry check failed: {error}", file=sys.stderr)
        return 1

    print("Selected Cortex-M0 entry reaches board, EEPROM, battery, and welcome executors")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
