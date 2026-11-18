#!/usr/bin/env python3
"""Validate the Cortex-M0 startup ELF and linker map."""

import argparse
import re
import struct
import sys
from pathlib import Path

FLASH_START = 0x00000000
FLASH_END = 0x0000F000
RAM_START = 0x20000000
RAM_END = 0x20004000
STACK_START = 0x20003800
STACK_SIZE = 0x800
INITIAL_SP = 0x20003FF0
VECTOR_SIZE = 48 * 4
PT_LOAD = 1
SHT_PROGBITS = 1
SHT_SYMTAB = 2
SHT_STRTAB = 3
SHT_NOBITS = 8
SHF_WRITE = 1
SHF_ALLOC = 2
SHF_EXECINSTR = 4
ELF32_HEADER = struct.Struct("<16sHHIIIIIHHHHHH")
ELF32_PROGRAM_HEADER = struct.Struct("<IIIIIIII")
ELF32_SECTION_HEADER = struct.Struct("<IIIIIIIIII")
ELF32_SYMBOL = struct.Struct("<IIIBBH")


class StartupError(ValueError):
    pass


def _read_c_string(data: bytes, offset: int) -> str:
    if offset >= len(data):
        raise StartupError("ELF string offset is out of range")
    end = data.find(b"\0", offset)
    if end < 0:
        raise StartupError("ELF string is not NUL terminated")
    return data[offset:end].decode("ascii", errors="replace")


def _parse_elf(path: Path) -> tuple[dict, dict, list, int]:
    data = path.read_bytes()
    if len(data) < ELF32_HEADER.size:
        raise StartupError("ELF is truncated before its ELF32 header")
    fields = ELF32_HEADER.unpack_from(data)
    ident = fields[0]
    if ident[:4] != b"\x7fELF" or ident[4] != 1 or ident[5] != 1:
        raise StartupError("input must be a little-endian ELF32 file")
    if fields[2] != 40:
        raise StartupError("ELF machine is not ARM")

    entry, program_offset, section_offset = fields[4], fields[5], fields[6]
    program_entry_size, program_count = fields[9], fields[10]
    section_entry_size, section_count, string_index = fields[11:14]
    if (
        program_entry_size < ELF32_PROGRAM_HEADER.size
        or program_count == 0
        or program_offset + program_entry_size * program_count > len(data)
    ):
        raise StartupError("ELF program-header table is malformed")
    if (
        section_entry_size != ELF32_SECTION_HEADER.size
        or section_count == 0
        or not 0 < string_index < section_count
        or section_offset + section_entry_size * section_count > len(data)
    ):
        raise StartupError("ELF section-header table is malformed")

    raw_sections = [
        ELF32_SECTION_HEADER.unpack_from(data, section_offset + index * section_entry_size)
        for index in range(section_count)
    ]
    name_section = raw_sections[string_index]
    if (
        name_section[1] != SHT_STRTAB
        or name_section[4] + name_section[5] > len(data)
    ):
        raise StartupError("ELF section-name string table is malformed")
    section_names = data[name_section[4] : name_section[4] + name_section[5]]
    sections = {
        _read_c_string(section_names, section[0]): section
        for section in raw_sections[1:]
    }

    symbols = {}
    for section in raw_sections:
        if section[1] != SHT_SYMTAB:
            continue
        string_table_index = section[6]
        if not 0 < string_table_index < section_count or section[9] < ELF32_SYMBOL.size:
            raise StartupError("ELF symbol table is malformed")
        string_table = raw_sections[string_table_index]
        if (
            string_table[1] != SHT_STRTAB
            or string_table[4] + string_table[5] > len(data)
            or section[4] + section[5] > len(data)
            or section[5] % section[9]
        ):
            raise StartupError("ELF symbol strings or entries are malformed")
        strings = data[string_table[4] : string_table[4] + string_table[5]]
        for offset in range(section[4], section[4] + section[5], section[9]):
            name, value, size, info, _, index = ELF32_SYMBOL.unpack_from(data, offset)
            symbol_name = _read_c_string(strings, name)
            if symbol_name:
                symbols[symbol_name] = (value, size, info, index)
    if not symbols:
        raise StartupError("ELF symbol table is required for startup validation")

    segments = []
    for index in range(program_count):
        header = ELF32_PROGRAM_HEADER.unpack_from(
            data, program_offset + index * program_entry_size
        )
        segment_type, file_offset, virtual, physical, file_size, memory_size = header[:6]
        if segment_type != PT_LOAD:
            continue
        if file_size > memory_size or file_offset + file_size > len(data):
            raise StartupError(f"PT_LOAD[{index}] is malformed or truncated")
        segments.append((virtual, physical, file_offset, file_size, memory_size))
    if not segments:
        raise StartupError("ELF has no PT_LOAD segments")
    return sections, symbols, segments, entry


def _check_map(path: Path) -> None:
    text = path.read_text(encoding="utf-8", errors="replace")
    expected_symbols = {
        "__vector_table_start": FLASH_START,
        "__vector_table_end": FLASH_START + VECTOR_SIZE,
        "__stack_start": STACK_START,
        "__stack_end": RAM_END,
        "__stack_top": RAM_END,
    }
    for name, expected in expected_symbols.items():
        match = re.search(
            rf"^\s*([0-9a-fA-F]+)\s+\S+\s+\S+\s+\S+\s+{re.escape(name)}\b",
            text,
            re.MULTILINE,
        )
        if match is None or int(match.group(1), 16) != expected:
            raise StartupError(f"linker map has an invalid or missing {name}")


def validate(elf_path: Path, map_path: Path) -> None:
    sections, symbols, segments, entry = _parse_elf(elf_path)
    for required in (".vector_table", ".text", ".data", ".bss", ".stack"):
        if required not in sections:
            raise StartupError(f"ELF is missing required section {required}")

    vector = sections[".vector_table"]
    if (
        vector[1] != SHT_PROGBITS
        or vector[2] & SHF_ALLOC == 0
        or vector[4] + vector[5] > elf_path.stat().st_size
    ):
        raise StartupError("vector table must be an allocated PROGBITS section")
    if vector[3] != FLASH_START or vector[5] != VECTOR_SIZE:
        raise StartupError("vector table must occupy the first 48 words of application flash")

    data = elf_path.read_bytes()
    stack_pointer, reset_vector = struct.unpack_from("<II", data, vector[4])
    reset_symbol = symbols.get("Reset_Handler")
    if reset_symbol is None:
        raise StartupError("ELF is missing Reset_Handler")
    reset_address = reset_symbol[0]
    if reset_symbol[2] & 0xF != 2 or reset_symbol[3] == 0:
        raise StartupError("Reset_Handler must be a defined function symbol")
    text = sections[".text"]
    if (
        text[2] & (SHF_ALLOC | SHF_EXECINSTR) != (SHF_ALLOC | SHF_EXECINSTR)
        or not (text[3] <= (reset_address & ~1) < text[3] + text[5])
    ):
        raise StartupError("Reset_Handler must reside in the allocated executable section")
    if stack_pointer != INITIAL_SP:
        raise StartupError("initial stack pointer does not match the target contract")
    if reset_vector & 1 == 0 or (reset_vector & ~1) != (reset_address & ~1):
        raise StartupError("reset vector is not the Thumb Reset_Handler address")
    if entry != reset_address:
        raise StartupError("ELF entry point is not Reset_Handler")
    data_section = sections[".data"]
    bss_section = sections[".bss"]
    if (
        data_section[1] != SHT_PROGBITS
        or data_section[2] & SHF_ALLOC == 0
        or bss_section[1] != SHT_NOBITS
        or bss_section[2] & (SHF_ALLOC | SHF_WRITE) != (SHF_ALLOC | SHF_WRITE)
    ):
        raise StartupError(".data and .bss must have the expected RAM section types")
    for name, section in ((".data", data_section), (".bss", bss_section)):
        if not (RAM_START <= section[3] <= section[3] + section[5] <= STACK_START):
            raise StartupError(f"{name} must reside in static RAM below the reserved stack")
    if data_section[5] and data_section[2] & SHF_WRITE == 0:
        raise StartupError(".data must be writable")

    for name, section in sections.items():
        if section[2] & SHF_ALLOC and section[5] and name != ".stack":
            address, size = section[3], section[5]
            if not (
                FLASH_START <= address <= address + size <= FLASH_END
                or RAM_START <= address <= address + size <= STACK_START
            ):
                raise StartupError(f"allocated section {name} is outside application memory")
    stack = sections[".stack"]
    if (
        stack[1] != SHT_NOBITS
        or stack[2] & (SHF_ALLOC | SHF_WRITE | SHF_EXECINSTR)
        != (SHF_ALLOC | SHF_WRITE)
        or stack[3] != STACK_START
        or stack[5] != STACK_SIZE
    ):
        raise StartupError("stack section must reserve the exact writable RAM-top range")

    for index, (virtual, physical, file_offset, file_size, memory_size) in enumerate(segments):
        if memory_size:
            if not (
                FLASH_START <= virtual <= virtual + memory_size <= FLASH_END
                or RAM_START <= virtual <= virtual + memory_size <= RAM_END
            ):
                raise StartupError(f"PT_LOAD[{index}] execution range is outside flash and RAM")
        if file_size:
            if not (FLASH_START <= physical <= physical + file_size <= FLASH_END):
                raise StartupError(f"PT_LOAD[{index}] load range is outside application flash")
            if virtual < FLASH_END and physical != virtual:
                raise StartupError(f"PT_LOAD[{index}] flash VMA and LMA do not match")
    for name, section in sections.items():
        if not section[2] & SHF_ALLOC or not section[5] or name == ".stack":
            continue
        address, file_offset, size = section[3], section[4], section[5]
        file_backed = section[1] != SHT_NOBITS
        if not any(
            0 <= address - virtual
            and address - virtual + size <= memory_size
            and (
                not file_backed
                or (
                    address - virtual + size <= segment_file_size
                    and file_offset == segment_file_offset + address - virtual
                )
            )
            for virtual, _, segment_file_offset, segment_file_size, memory_size in segments
        ):
            kind = "file-backed " if file_backed else ""
            raise StartupError(
                f"allocated section {name} is not covered by a {kind}PT_LOAD segment"
            )
    _check_map(map_path)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--elf", required=True, type=Path)
    parser.add_argument("--map", required=True, type=Path)
    arguments = parser.parse_args(argv)
    try:
        validate(arguments.elf, arguments.map)
    except (OSError, StartupError, struct.error) as error:
        print(f"startup layout check failed: {error}", file=sys.stderr)
        return 1
    print(
        "Startup layout valid: entry/reset vector, initial SP, 48 vectors, "
        "sections, flash/RAM ranges, and linker map"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
