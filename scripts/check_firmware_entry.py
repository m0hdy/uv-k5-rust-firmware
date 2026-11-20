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
    r"^[ \t]*([0-9a-fA-F]+):[ \t]+(?:(?:[0-9a-fA-F]{4,8})[ \t]+)*"
    r"([A-Za-z][A-Za-z0-9.]*)[ \t]*(.*?)$",
    re.MULTILINE,
)
CONDITIONAL_BRANCH = re.compile(
    r"b(?:eq|ne|cs|cc|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)(?:\.n|\.w)?"
)
EEPROM_SUCCESS_TAG = 2
PREDICATE_PRESERVING_INSTRUCTIONS = {"nop", "str", "strb", "strh"}
MAX_ABSTRACT_VALUES = 8
MAX_ERROR_COPY_BYTES = 64


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
    addresses = {address for address, _, _ in rows}
    if mnemonic == "bl":
        successors = [following] if following is not None else []
        if target in addresses:
            successors.append(target)
        return successors
    if mnemonic == "blx":
        return []
    if mnemonic in {"b", "b.n", "b.w", "bal", "bal.n", "bal.w"}:
        return [target] if target is not None else []
    if CONDITIONAL_BRANCH.fullmatch(mnemonic) or mnemonic in {"cbz", "cbnz"}:
        return [address for address in (target, following) if address is not None]
    if mnemonic.startswith("bx") or (
        mnemonic.startswith("pop") and re.search(r"\bpc\b", operands)
    ) or re.match(r"\s*pc\s*,", operands) or mnemonic.startswith("udf"):
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


def is_mmio_address(address: int) -> bool:
    return 0x40000000 <= address < 0x60000000 or 0xE0000000 <= address < 0xE0100000


def written_back_registers(operands: str) -> set[int]:
    clean_operands = operands.split("@")[0].strip()
    registers = {
        int(match)
        for match in re.findall(r"(?:^|\[)r([0-7])!", clean_operands)
    }
    registers.update(
        int(match)
        for match in re.findall(
            r"\[r([0-7])(?:,\s*[^\]]+)?\](?:!|,\s*#)", clean_operands
        )
    )
    return registers


def clobbered_address_registers(mnemonic: str, operands: str) -> set[int]:
    registers = written_back_registers(operands)
    if mnemonic.startswith("ldm"):
        register_list = re.search(r"\{([^}]*)\}", operands)
        if register_list:
            registers.update(
                int(match)
                for match in re.findall(r"\br([0-7])\b", register_list.group(1))
            )
    return registers


def function_has_mmio_store(body: str) -> bool:
    rows = instructions(body)
    if not rows:
        return False
    live = reachable_instruction_addresses(rows)
    indices = {address: index for index, (address, _, _) in enumerate(rows)}
    words = {
        int(address, 16): int(value, 16)
        for address, value in re.findall(
            r"(?m)^\s*([0-9a-fA-F]+):\s+(?:[0-9a-fA-F]+\s+)?"
            r"\.word\s+0x([0-9a-fA-F]+)\b",
            body,
        )
    }
    states: dict[int, tuple[frozenset[int] | None, ...]] = {
        rows[0][0]: (None,) * 8
    }
    pending = [rows[0][0]]
    non_writing = {
        "cmp",
        "cpsid",
        "nop",
        "push",
        "tst",
        "udf",
        "wfi",
    }
    while pending:
        address = pending.pop()
        state = list(states[address])
        _, mnemonic, operands = rows[indices[address]]
        for register in clobbered_address_registers(mnemonic, operands):
            state[register] = None
        store = re.fullmatch(
            r"r[0-7],\s*\[(r[0-7])(?:,\s*#(\d+))?\]",
            operands.split("@")[0].strip(),
        )
        if mnemonic.startswith("str") and store:
            base = state[int(store.group(1)[1])]
            offset = int(store.group(2) or 0)
            if base and all(is_mmio_address(value + offset) for value in base):
                return True

        destination = re.match(r"\s*r([0-7])\s*,", operands)
        if mnemonic == "bl":
            state[:4] = [None] * 4
        elif mnemonic == "pop":
            for register in re.findall(r"\br([0-7])\b", operands):
                state[int(register)] = None
        elif destination:
            register = int(destination.group(1))
            pc_load = re.fullmatch(
                r"r[0-7],\s*\[pc,\s*#(\d+)\]",
                operands.split("@")[0].strip(),
            )
            copied = re.fullmatch(r"r[0-7],\s*r([0-7])", operands.strip())
            added = re.fullmatch(
                r"r[0-7],\s*r([0-7]),\s*#(\d+)",
                operands.split("@")[0].strip(),
            )
            immediate = re.fullmatch(r"r[0-7],\s*#(\d+)", operands.strip())
            if mnemonic == "ldr" and pc_load:
                literal = ((address + 4) & ~3) + int(pc_load.group(1))
                value = words.get(literal)
                state[register] = (
                    frozenset({value}) if value is not None else None
                )
            elif mnemonic == "mov" and copied:
                state[register] = state[int(copied.group(1))]
            elif mnemonic in {"add", "adds"} and added:
                source = state[int(added.group(1))]
                offset = int(added.group(2))
                values = (
                    frozenset(value + offset for value in source)
                    if source is not None
                    else None
                )
                state[register] = (
                    values
                    if values is not None
                    and len(values) <= MAX_ABSTRACT_VALUES
                    else None
                )
            elif mnemonic == "movs" and immediate:
                state[register] = frozenset({int(immediate.group(1))})
            elif not (
                mnemonic.startswith("str")
                or CONDITIONAL_BRANCH.fullmatch(mnemonic)
                or mnemonic in non_writing
            ):
                state[register] = None

        outgoing = tuple(state)
        for successor in instruction_successors(rows, indices[address]):
            if successor not in live:
                continue
            current = states.get(successor)
            if current is None:
                merged = outgoing
            else:
                merged = tuple(
                    None
                    if left is None or right is None
                    else (
                        values
                        if len(values := left | right) <= MAX_ABSTRACT_VALUES
                        else None
                    )
                    for left, right in zip(current, outgoing)
                )
            if merged != current:
                states[successor] = merged
                pending.append(successor)
    return False


def has_mmio_store(functions: dict[str, str], reached: set[str], start: str) -> bool:
    pending = [start]
    visited = set()
    while pending:
        current = pending.pop()
        if current in visited:
            continue
        visited.add(current)
        if function_has_mmio_store(functions[current]):
            return True
        pending.extend(
            target
            for target in call_targets(functions[current])
            if target in reached and target not in visited
        )
    return False


def error_effect_kinds(body: str) -> tuple[dict[int, str], dict[int, bool]]:
    rows = instructions(body)
    live = reachable_instruction_addresses(rows)
    indices = {address: index for index, (address, _, _) in enumerate(rows)}
    words = {
        int(address, 16): int(value, 16)
        for address, value in re.findall(
            r"(?m)^\s*([0-9a-fA-F]+):\s+(?:[0-9a-fA-F]+\s+)?"
            r"\.word\s+0x([0-9a-fA-F]+)\b",
            body,
        )
    }
    Value = frozenset[tuple[str, int]]
    initial_registers: tuple[Value | None, ...] = tuple(
        (
            frozenset({("result" if register == 0 else "data", register)})
            if register < 4
            else None
        )
        for register in range(8)
    )
    states: dict[
        int,
        tuple[
            tuple[Value | None, ...],
            tuple[tuple[int, Value], ...],
        ],
    ] = {rows[0][0]: (initial_registers, ())}
    pending = [rows[0][0]]
    stores: dict[int, str] = {}
    copy_calls: dict[int, bool] = {}

    def add_offset(value: Value | None, offset: int) -> Value | None:
        if value is None:
            return None
        shifted = frozenset((kind, base + offset) for kind, base in value)
        return shifted if len(shifted) <= MAX_ABSTRACT_VALUES else None

    def merge_value(left: Value | None, right: Value | None) -> Value | None:
        if left is None or right is None:
            return None
        merged = left | right
        return merged if len(merged) <= MAX_ABSTRACT_VALUES else None

    while pending:
        address = pending.pop()
        register_state, memory_state = states[address]
        registers = list(register_state)
        memory = dict(memory_state)
        _, mnemonic, operands = rows[indices[address]]
        clean_operands = operands.split("@")[0].strip()
        target = re.search(r"<(.+)>", operands)
        if (
            mnemonic.startswith("bl")
            and target
            and "__aeabi_memcpy" in target.group(1)
        ):
            destination = registers[0]
            length = registers[2]
            safe_copy = bool(
                destination
                and {kind for kind, _ in destination}
                in ({"stack"}, {"result"})
                and length
                and {kind for kind, _ in length} == {"const"}
                and all(
                    0 < value <= MAX_ERROR_COPY_BYTES
                    for _, value in length
                )
            )
            copy_calls[address] = safe_copy
            if not safe_copy:
                memory.clear()
            elif {kind for kind, _ in destination} == {"stack"}:
                memory = {
                    slot: value
                    for slot, value in memory.items()
                    if all(
                        slot + 4 <= copied_to
                        or copied_to + copied_length <= slot
                        for _, copied_to in destination
                        for _, copied_length in length
                    )
                }
        multiple_store = re.fullmatch(
            r"r([0-7])!?,\s*\{[^}]+\}", clean_operands
        )
        if mnemonic.startswith("stm"):
            stores[address] = "unknown"
            if multiple_store is None:
                memory.clear()
            else:
                base = registers[int(multiple_store.group(1))]
                kinds = {kind for kind, _ in base} if base is not None else set()
                if base and kinds == {"stack"}:
                    stores[address] = "stack"
                    memory.clear()
                elif base and kinds in ({"data"}, {"result"}):
                    stores[address] = "data"
                elif base and kinds == {"const"} and all(
                    is_mmio_address(value) for _, value in base
                ):
                    stores[address] = "mmio"
                else:
                    memory.clear()
        store = re.fullmatch(
            r"r([0-7]),\s*\[(sp|r[0-7])(?:,\s*#(\d+))?\]",
            clean_operands,
        )
        if mnemonic.startswith("str"):
            stores[address] = "unknown"
            if store is None:
                memory.clear()
        if mnemonic.startswith("str") and store:
            base_name = store.group(2)
            offset = int(store.group(3) or 0)
            base = (
                frozenset({("stack", offset)})
                if base_name == "sp"
                else add_offset(registers[int(base_name[1])], offset)
            )
            kinds = {kind for kind, _ in base} if base is not None else set()
            if base and kinds == {"stack"}:
                stores[address] = "stack"
            elif base and kinds in ({"data"}, {"result"}):
                stores[address] = "data"
            elif base and kinds == {"const"} and all(
                is_mmio_address(value) for _, value in base
            ):
                stores[address] = "mmio"
            else:
                stores[address] = "unknown"
            if base is None:
                memory.clear()
            elif kinds == {"stack"}:
                width = {"str": 4, "strh": 2, "strb": 1}.get(mnemonic)
                if width is None:
                    memory.clear()
                else:
                    stack_offsets = {value for _, value in base}
                    memory = {
                        slot: value
                        for slot, value in memory.items()
                        if all(
                            slot + 4 <= written or written + width <= slot
                            for written in stack_offsets
                        )
                    }
                    if mnemonic == "str" and len(stack_offsets) == 1:
                        source = registers[int(store.group(1))]
                        if source is not None:
                            memory[next(iter(stack_offsets))] = source
            elif stores[address] == "unknown":
                memory.clear()

        for register in clobbered_address_registers(mnemonic, operands):
            registers[register] = None
        destination = re.match(r"\s*r([0-7])\s*,", operands)
        if mnemonic in {"bl", "blx"}:
            registers[:4] = [None] * 4
        elif mnemonic == "pop":
            for register in re.findall(r"\br([0-7])\b", operands):
                registers[int(register)] = None
        elif re.fullmatch(r"(?:add|sub)\s+sp,\s*#\d+.*", f"{mnemonic} {clean_operands}"):
            memory.clear()
        elif destination:
            register = int(destination.group(1))
            pc_load = re.fullmatch(r"r[0-7],\s*\[pc,\s*#(\d+)\]", clean_operands)
            sp_load = re.fullmatch(r"r[0-7],\s*\[sp,\s*#(\d+)\]", clean_operands)
            copied = re.fullmatch(r"r[0-7],\s*r([0-7])", clean_operands)
            added = re.fullmatch(
                r"r[0-7],\s*(sp|r[0-7]),\s*#(\d+)", clean_operands
            )
            added_in_place = re.fullmatch(r"r[0-7],\s*#(\d+)", clean_operands)
            immediate = re.fullmatch(r"r[0-7],\s*#(\d+)", clean_operands)
            if mnemonic == "ldr" and pc_load:
                literal = ((address + 4) & ~3) + int(pc_load.group(1))
                value = words.get(literal)
                registers[register] = (
                    frozenset({("const", value)}) if value is not None else None
                )
            elif mnemonic == "ldr" and sp_load:
                registers[register] = memory.get(int(sp_load.group(1)))
            elif mnemonic == "mov" and copied:
                registers[register] = registers[int(copied.group(1))]
            elif mnemonic in {"add", "adds"} and added:
                source = (
                    frozenset({("stack", 0)})
                    if added.group(1) == "sp"
                    else registers[int(added.group(1)[1])]
                )
                registers[register] = add_offset(source, int(added.group(2)))
            elif mnemonic in {"add", "adds"} and added_in_place:
                registers[register] = add_offset(
                    registers[register], int(added_in_place.group(1))
                )
            elif mnemonic == "movs" and immediate:
                registers[register] = frozenset(
                    {("const", int(immediate.group(1)))}
                )
            elif not (
                mnemonic.startswith("str")
                or CONDITIONAL_BRANCH.fullmatch(mnemonic)
                or mnemonic in {"cmp", "cpsid", "nop", "push", "tst", "udf", "wfi"}
            ):
                registers[register] = None

        outgoing = (tuple(registers), tuple(sorted(memory.items())))
        for successor in instruction_successors(rows, indices[address]):
            if successor not in live:
                continue
            current = states.get(successor)
            if current is None:
                merged = outgoing
            else:
                current_registers, current_memory_items = current
                merged_registers = tuple(
                    merge_value(left, right)
                    for left, right in zip(current_registers, outgoing[0])
                )
                current_memory = dict(current_memory_items)
                outgoing_memory = dict(outgoing[1])
                merged_memory = {
                    offset: merged_value
                    for offset in current_memory.keys() & outgoing_memory.keys()
                    if (
                        merged_value := merge_value(
                            current_memory[offset], outgoing_memory[offset]
                        )
                    )
                    is not None
                }
                merged = (merged_registers, tuple(sorted(merged_memory.items())))
            if merged != current:
                states[successor] = merged
                pending.append(successor)
    return stores, copy_calls


def error_store_kinds(body: str) -> dict[int, str]:
    return error_effect_kinds(body)[0]


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


def unsupported_control_transfer(mnemonic: str, operands: str) -> bool:
    if mnemonic == "blx" or re.match(r"\s*pc\s*,", operands):
        return True
    return (
        mnemonic.startswith("b")
        and mnemonic not in {
            "b",
            "b.n",
            "b.w",
            "bal",
            "bal.n",
            "bal.w",
            "bl",
            "bx",
            "bx.n",
            "bkpt",
            "bic",
            "bics",
        }
        and not CONDITIONAL_BRANCH.fullmatch(mnemonic)
    )


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
    for address, mnemonic, operands in rows[:call_index]:
        if address not in live:
            continue
        target = branch_target(operands)
        if mnemonic == "bl" and target in indices:
            raise EntryError(
                "EEPROM result-address proof does not permit intra-function calls"
            )
        if unsupported_control_transfer(mnemonic, operands):
            raise EntryError(
                "EEPROM result-address proof encountered unsupported control transfer"
            )
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


def is_supported_return(mnemonic: str, operands: str) -> bool:
    clean_operands = operands.split("@")[0].strip()
    return (
        (mnemonic == "bx" and clean_operands == "lr")
        or (
            mnemonic == "pop"
            and re.search(r"\bpc\b", clean_operands) is not None
        )
    )


def require_fail_closed_terminal(body: str) -> None:
    rows = instructions(body)
    live = reachable_instruction_addresses(rows)
    indices = {address: index for index, (address, _, _) in enumerate(rows)}
    predecessors = {address: set() for address in live}
    for address in live:
        for successor in instruction_successors(rows, indices[address]):
            if successor in live:
                predecessors[successor].add(address)
    dominators = {
        address: {address} if address == rows[0][0] else set(live)
        for address in live
    }
    changed = True
    while changed:
        changed = False
        for address in live - {rows[0][0]}:
            incoming = predecessors[address]
            updated = {address} | (
                set.intersection(*(dominators[item] for item in incoming))
                if incoming
                else set()
            )
            if updated != dominators[address]:
                dominators[address] = updated
                changed = True

    masks = {
        address
        for address, mnemonic, operands in rows
        if address in live and (mnemonic, operands.strip()) == ("cpsid", "i")
    }
    parks = []
    for address, mnemonic, _ in rows:
        if address not in live or mnemonic != "wfi":
            continue
        from_wfi = reachable_instruction_addresses(rows, [address])
        component = {
            candidate
            for candidate in from_wfi
            if address in reachable_instruction_addresses(rows, [candidate])
        }
        if not component or not any(
            all(mask in dominators[node] for node in component)
            for mask in masks
        ):
            continue
        if any(
            successor not in component
            for node in component
            for successor in instruction_successors(rows, indices[node])
        ):
            continue
        if len(component) == 1 and address not in instruction_successors(
            rows, indices[address]
        ):
            continue
        parks.append(component)

    for address, mnemonic, operands in rows:
        if address not in live:
            continue
        target = branch_target(operands)
        if (
            mnemonic in {"bl", "blx"}
            or mnemonic.startswith("bx")
            or (mnemonic.startswith("pop") and re.search(r"\bpc\b", operands))
            or re.match(r"\s*pc\s*,", operands)
            or (
                mnemonic.startswith("b")
                and target is not None
                and target not in indices
            )
        ):
            raise EntryError("firmware_fail_closed has a reachable return or escape")
    if not parks or not any(
        all(
            reachable_instruction_addresses(rows, [address]) & component
            for address in live
        )
        for component in parks
    ):
        raise EntryError(
            "firmware_fail_closed must reach an interrupt-masked closed WFI loop"
        )


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
    stores, safe_copy_calls = error_effect_kinds(body)
    guarded_paths = []
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
        allowed_error_calls = (
            "__aeabi_memcpy",
            "handoff_to_keypad",
            "i2c_handoff",
            "I2cTransport>::stop",
        )
        for address, mnemonic, operands in rows:
            if address not in error_path or not mnemonic.startswith("bl"):
                continue
            target = re.search(r"<(.+)>", operands)
            if target is None or not any(
                allowed in target.group(1) for allowed in allowed_error_calls
            ):
                raise EntryError(
                    f"{called_fragment} first-error path reaches a later effect"
                )
            if (
                "__aeabi_memcpy" in target.group(1)
                and not safe_copy_calls.get(address, False)
            ):
                raise EntryError(
                    f"{called_fragment} first-error path reaches an unapproved copy effect"
                )
        short_circuit = error_successor
        short_path = reachable_instruction_addresses(rows, [short_circuit])
        returns = {
            address
            for address, mnemonic, operands in rows
            if is_supported_return(mnemonic, operands)
        }
        for address, mnemonic, operands in rows:
            if address not in short_path:
                continue
            target = branch_target(operands)
            if (
                mnemonic.startswith("bx")
                and not is_supported_return(mnemonic, operands)
                or mnemonic == "blx"
                or (
                    mnemonic.startswith("pop")
                    and re.search(r"\bpc\b", operands)
                    and not is_supported_return(mnemonic, operands)
                )
                or re.match(r"\s*pc\s*,", operands)
                or (
                    mnemonic.startswith("b")
                    and not mnemonic.startswith("bl")
                    and target is not None
                    and target not in indices
                )
            ):
                raise EntryError(
                    f"{called_fragment} first-error path has an unsupported return or escape"
                )
        if not short_path & returns:
            raise EntryError(
                f"{called_fragment} first-error path does not return from the coordinator"
            )
        guarded_paths.append((error_path, success_path))

    common_handoff = set.intersection(
        *(error_path for error_path, _ in guarded_paths),
        guarded_paths[-1][1],
    )
    for error_path, _ in guarded_paths:
        for address in error_path:
            kind = stores.get(address)
            if kind is None or kind in {"stack", "data"}:
                continue
            if kind == "mmio" and address in common_handoff:
                continue
            raise EntryError(
                f"{called_fragment} first-error path reaches an unapproved store effect"
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
    require_fail_closed_terminal(fail_closed)

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
        if not has_mmio_store(functions, startup_path, executor):
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
