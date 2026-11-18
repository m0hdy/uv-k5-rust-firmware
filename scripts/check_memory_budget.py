#!/usr/bin/env python3
"""Check loadable ELF ranges and raw image budgets for UV-K5 firmware."""

import argparse
import struct
import sys
from dataclasses import dataclass
from pathlib import Path

FLASH_START = 0x00000000
FLASH_END = 0x0000F000
RAM_START = 0x20000000
RAM_END = 0x20004000
MAX_RAW_BYTES = FLASH_END - FLASH_START
REQUIRED_STACK_RESERVATION = 0x800
PT_LOAD = 1
ELF32_HEADER = struct.Struct("<16sHHIIIIIHHHHHH")
ELF32_PROGRAM_HEADER = struct.Struct("<IIIIIIII")
ELF32_SECTION_HEADER = struct.Struct("<IIIIIIIIII")
SHT_STRTAB = 3
SHT_NOBITS = 8
SHF_WRITE = 1
SHF_ALLOC = 2
SHF_EXECINSTR = 4


class BudgetError(ValueError):
    pass


@dataclass(frozen=True)
class Segment:
    index: int
    virtual_address: int
    physical_address: int
    file_offset: int
    file_size: int
    memory_size: int
    file_data: bytes


@dataclass(frozen=True)
class Section:
    name: str
    kind: int
    address: int
    size: int
    load_address: int | None


@dataclass
class Report:
    segments: list[Segment]
    sections: list[Section]
    flash_load_bytes: int
    ram_static_bytes: int
    raw_bytes: int
    packed_bytes: int | None
    errors: list[str]


def validate_allocated_sections(
    data: bytes, fields: tuple, segments: list[Segment]
) -> tuple[list[Section], bool]:
    offset, entry_size, count, string_index = (
        fields[6], fields[11], fields[12], fields[13]
    )
    if offset == 0:
        raise BudgetError("ELF section headers are required to validate allocated sections")
    if (
        entry_size != ELF32_SECTION_HEADER.size
        or count == 0
        or not 0 < string_index < count
        or offset + entry_size * count > len(data)
    ):
        raise BudgetError("ELF section-header table is truncated or unsupported")
    sections = [
        ELF32_SECTION_HEADER.unpack_from(data, offset + index * entry_size)
        for index in range(count)
    ]
    string_offset, string_size = sections[string_index][4:6]
    if sections[string_index][1] != SHT_STRTAB or string_offset + string_size > len(data):
        raise BudgetError("ELF section-name string table is malformed")
    names = data[string_offset : string_offset + string_size]
    allocated = []
    reported_sections = []
    linked_stack = False
    stack_start = RAM_END - REQUIRED_STACK_RESERVATION
    for section in sections:
        name_offset, kind, flags, address, file_offset, size = section[:6]
        end = names.find(b"\0", name_offset)
        if end == -1:
            raise BudgetError("ELF section name is malformed")
        name = names[name_offset:end].decode("ascii", errors="backslashreplace")
        if name == ".stack":
            if linked_stack:
                raise BudgetError("ELF must contain at most one .stack section")
            if (
                kind != SHT_NOBITS
                or flags & (SHF_ALLOC | SHF_WRITE | SHF_EXECINSTR) != (SHF_ALLOC | SHF_WRITE)
                or address != stack_start
                or size != REQUIRED_STACK_RESERVATION
            ):
                raise BudgetError(
                    "linked .stack must be writable, allocated NOBITS in the exact reserved range"
                )
            linked_stack = True
            reported_sections.append(Section(name, kind, address, size, None))
        elif flags & SHF_ALLOC and size:
            allocated.append((name, kind, address, file_offset, size))

    for name, kind, address, file_offset, size in allocated:
        if not (
            in_region(address, size, FLASH_START, FLASH_END)
            or in_region(address, size, RAM_START, RAM_END)
        ):
            raise BudgetError(f"allocated section {name} is outside flash and RAM")
        if overlaps((address, address + size), (stack_start, RAM_END)):
            raise BudgetError(f"allocated section {name} overlaps the reserved stack")
        for segment in segments:
            delta = address - segment.virtual_address
            if 0 <= delta and delta + size <= segment.memory_size and (
                kind == SHT_NOBITS
                or (
                    delta + size <= segment.file_size
                    and file_offset == segment.file_offset + delta
                )
            ):
                load_address = (
                    segment.physical_address + delta
                    if kind != SHT_NOBITS
                    else None
                )
                break
        else:
            raise BudgetError(f"allocated section {name} is not covered by a PT_LOAD segment")
        reported_sections.append(
            Section(name, kind, address, size, load_address)
        )
    return reported_sections, linked_stack


def parse_elf(path: Path) -> tuple[list[Segment], list[Section], bool]:
    data = path.read_bytes()
    if len(data) < ELF32_HEADER.size:
        raise BudgetError("ELF is truncated before its ELF32 header")

    fields = ELF32_HEADER.unpack_from(data)
    ident = fields[0]
    if ident[:4] != b"\x7fELF":
        raise BudgetError("input is not an ELF file")
    if ident[4] != 1:
        raise BudgetError("only ELF32 files are supported")
    if ident[5] != 1:
        raise BudgetError("only little-endian ELF files are supported")
    if fields[2] != 40:
        raise BudgetError("ELF machine is not ARM")

    program_header_offset = fields[5]
    program_header_entry_size = fields[9]
    program_header_count = fields[10]
    elf_header_size = fields[8]
    if elf_header_size < ELF32_HEADER.size or elf_header_size > len(data):
        raise BudgetError("ELF header size is malformed")
    if program_header_count == 0:
        raise BudgetError("ELF has no program headers")
    if program_header_entry_size < ELF32_PROGRAM_HEADER.size:
        raise BudgetError("ELF program-header entries are malformed")
    table_end = program_header_offset + program_header_entry_size * program_header_count
    if program_header_offset < elf_header_size or table_end > len(data):
        raise BudgetError("ELF program-header table is truncated or malformed")

    segments = []
    for index in range(program_header_count):
        offset = program_header_offset + index * program_header_entry_size
        header = ELF32_PROGRAM_HEADER.unpack_from(data, offset)
        segment_type, file_offset, virtual_address, physical_address, file_size, memory_size = header[:6]
        if segment_type != PT_LOAD:
            continue
        if file_size > memory_size:
            raise BudgetError(f"PT_LOAD[{index}] file size exceeds memory size")
        if file_offset + file_size > len(data):
            raise BudgetError(f"PT_LOAD[{index}] file data is truncated")
        if physical_address + file_size > 0x1_0000_0000:
            raise BudgetError(f"PT_LOAD[{index}] load range overflows ELF32 address space")
        if virtual_address + memory_size > 0x1_0000_0000:
            raise BudgetError(f"PT_LOAD[{index}] execution range overflows ELF32 address space")
        segments.append(
            Segment(
                index,
                virtual_address,
                physical_address,
                file_offset,
                file_size,
                memory_size,
                data[file_offset : file_offset + file_size],
            )
        )

    if not segments:
        raise BudgetError("ELF has no PT_LOAD segments")
    sections, has_linked_stack = validate_allocated_sections(data, fields, segments)
    return segments, sections, has_linked_stack


def in_region(start: int, size: int, region_start: int, region_end: int) -> bool:
    return start >= region_start and start + size <= region_end


def overlaps(left: tuple[int, int], right: tuple[int, int]) -> bool:
    return left[0] < right[1] and right[0] < left[1]


def find_overlaps(ranges: list[tuple[int, int, int]], label: str) -> list[str]:
    errors = []
    ordered = sorted(ranges)
    for previous, current in zip(ordered, ordered[1:]):
        if overlaps((previous[0], previous[1]), (current[0], current[1])):
            errors.append(
                f"{label} ranges overlap: PT_LOAD[{previous[2]}] "
                f"[0x{previous[0]:08x},0x{previous[1]:08x}) and "
                f"PT_LOAD[{current[2]}] [0x{current[0]:08x},0x{current[1]:08x})"
            )
    return errors


def analyze(
    elf_path: Path,
    raw_path: Path,
    stack_reservation: int,
    packed_path: Path | None = None,
) -> Report:
    segments, sections, has_linked_stack = parse_elf(elf_path)
    raw_data = raw_path.read_bytes()
    raw_bytes = len(raw_data)
    packed_bytes = packed_path.stat().st_size if packed_path is not None else None
    errors = []
    load_ranges = []
    flash_memory_ranges = []
    ram_memory_ranges = []

    if stack_reservation != REQUIRED_STACK_RESERVATION:
        errors.append(
            f"stack reservation must be exactly {REQUIRED_STACK_RESERVATION} bytes "
            f"(0x{REQUIRED_STACK_RESERVATION:x})"
        )

    for segment in segments:
        if segment.file_size:
            load_start = segment.physical_address
            load_end = load_start + segment.file_size
            load_ranges.append((load_start, load_end, segment.index))
            if not in_region(load_start, segment.file_size, FLASH_START, FLASH_END):
                errors.append(
                    f"PT_LOAD[{segment.index}] load/file range "
                    f"[0x{load_start:08x},0x{load_end:08x}) is outside flash "
                    f"[0x{FLASH_START:08x},0x{FLASH_END:08x})"
                )
            if (
                segment.memory_size
                and in_region(
                    segment.virtual_address,
                    segment.memory_size,
                    FLASH_START,
                    FLASH_END,
                )
                and segment.virtual_address != segment.physical_address
            ):
                errors.append(
                    f"PT_LOAD[{segment.index}] file-backed flash XIP VMA "
                    f"0x{segment.virtual_address:08x} does not match LMA "
                    f"0x{segment.physical_address:08x}"
                )
            if load_end > raw_bytes:
                errors.append(
                    f"raw payload ({raw_bytes} bytes) does not cover "
                    f"PT_LOAD[{segment.index}] load range ending at 0x{load_end:08x}"
                )

        if segment.memory_size:
            memory_start = segment.virtual_address
            memory_end = memory_start + segment.memory_size
            if in_region(memory_start, segment.memory_size, FLASH_START, FLASH_END):
                flash_memory_ranges.append((memory_start, memory_end, segment.index))
            elif in_region(memory_start, segment.memory_size, RAM_START, RAM_END):
                ram_memory_ranges.append((memory_start, memory_end, segment.index))
            else:
                errors.append(
                    f"PT_LOAD[{segment.index}] execution/memory range "
                    f"[0x{memory_start:08x},0x{memory_end:08x}) is outside flash "
                    f"[0x{FLASH_START:08x},0x{FLASH_END:08x}) and RAM "
                    f"[0x{RAM_START:08x},0x{RAM_END:08x})"
                )
                if memory_start < FLASH_END and memory_end > FLASH_START:
                    flash_memory_ranges.append((memory_start, memory_end, segment.index))
                if memory_start < RAM_END and memory_end > RAM_START:
                    ram_memory_ranges.append((memory_start, memory_end, segment.index))

    flash_file_segments = [
        segment
        for segment in segments
        if segment.file_size
        and in_region(
            segment.physical_address,
            segment.file_size,
            FLASH_START,
            FLASH_END,
        )
    ]
    if not flash_file_segments:
        errors.append("ELF has no file-backed flash load content")
    else:
        lowest_load = min(segment.physical_address for segment in flash_file_segments)
        if lowest_load != FLASH_START:
            errors.append(
                f"raw image must start at application origin 0x{FLASH_START:08x}; "
                f"lowest file-backed load address is 0x{lowest_load:08x}"
            )
        load_span_end = max(
            segment.physical_address + segment.file_size
            for segment in flash_file_segments
        )
        if raw_bytes != load_span_end - FLASH_START:
            errors.append(
                f"raw payload span is {raw_bytes} bytes; expected "
                f"{load_span_end - FLASH_START} bytes from application origin "
                f"through the last file-backed load byte"
            )
        if (
            lowest_load == FLASH_START
            and all(
                segment.file_size == 0
                or in_region(
                    segment.physical_address,
                    segment.file_size,
                    FLASH_START,
                    FLASH_END,
                )
                for segment in segments
            )
            and not find_overlaps(load_ranges, "Flash load/file")
            and raw_bytes == load_span_end - FLASH_START
        ):
            expected_raw = bytearray(load_span_end - FLASH_START)
            for segment in flash_file_segments:
                start = segment.physical_address - FLASH_START
                expected_raw[start : start + segment.file_size] = segment.file_data
            if raw_data != expected_raw:
                mismatch = next(
                    index
                    for index, (actual, expected) in enumerate(
                        zip(raw_data, expected_raw)
                    )
                    if actual != expected
                )
                errors.append(
                    f"raw payload byte at offset 0x{mismatch:x} does not match "
                    "the ELF load image (including zero-filled gaps)"
                )

    errors.extend(find_overlaps(load_ranges, "Flash load/file"))
    errors.extend(find_overlaps(flash_memory_ranges, "Flash execution/memory"))
    errors.extend(find_overlaps(ram_memory_ranges, "RAM execution/memory"))
    for load_start, load_end, load_index in load_ranges:
        for memory_start, memory_end, memory_index in flash_memory_ranges:
            if load_index != memory_index and overlaps(
                (load_start, load_end), (memory_start, memory_end)
            ):
                errors.append(
                    f"PT_LOAD[{load_index}] flash load/file range "
                    f"[0x{load_start:08x},0x{load_end:08x}) overlaps "
                    f"PT_LOAD[{memory_index}] flash execution/memory range "
                    f"[0x{memory_start:08x},0x{memory_end:08x})"
                )

    stack_start = RAM_END - REQUIRED_STACK_RESERVATION
    static_ram_end = stack_start if has_linked_stack else RAM_END
    ram_bytes = sum(
        max(
            0,
            min(segment.virtual_address + segment.memory_size, static_ram_end)
            - max(segment.virtual_address, RAM_START),
        )
        for segment in segments
    )
    flash_bytes = sum(
        max(
            0,
            min(segment.physical_address + segment.file_size, FLASH_END)
            - max(segment.physical_address, FLASH_START),
        )
        for segment in segments
    )
    stack_range = (stack_start, RAM_END)
    for start, end, index in ram_memory_ranges:
        if not has_linked_stack and overlaps((start, end), stack_range):
            errors.append(
                f"PT_LOAD[{index}] RAM range [0x{start:08x},0x{end:08x}) "
                f"overlaps the reserved stack [0x{stack_start:08x},0x{RAM_END:08x})"
            )
    if has_linked_stack:
        for segment in segments:
            start = segment.virtual_address
            if segment.file_size and overlaps((start, start + segment.file_size), stack_range):
                errors.append(f"PT_LOAD[{segment.index}] initializes bytes in the reserved stack")

    if raw_bytes == 0:
        errors.append("raw payload is empty")
    if raw_bytes > MAX_RAW_BYTES:
        errors.append(
            f"raw payload is {raw_bytes} bytes; maximum is {MAX_RAW_BYTES} bytes"
        )
    if ram_bytes + stack_reservation > RAM_END - RAM_START:
        errors.append(
            f"RAM use plus stack is {ram_bytes + stack_reservation} bytes; "
            f"maximum is {RAM_END - RAM_START} bytes"
        )

    return Report(
        segments, sections, flash_bytes, ram_bytes, raw_bytes, packed_bytes, errors
    )


def format_report(report: Report, stack_reservation: int) -> str:
    lines = [
        f"Flash load bytes: {report.flash_load_bytes} / {MAX_RAW_BYTES} "
        f"({MAX_RAW_BYTES - report.flash_load_bytes} remaining)",
        f"Raw payload: {report.raw_bytes} / {MAX_RAW_BYTES} "
        f"({MAX_RAW_BYTES - report.raw_bytes} remaining)",
        f"RAM static use: {report.ram_static_bytes} bytes",
        f"Reserved stack: {stack_reservation} bytes",
        f"RAM use plus stack: {report.ram_static_bytes + stack_reservation} / "
        f"{RAM_END - RAM_START} "
        f"({RAM_END - RAM_START - report.ram_static_bytes - stack_reservation} remaining)",
        (
            f"Packed container: {report.packed_bytes} bytes (reported separately; "
            "not subject to the raw payload limit)"
            if report.packed_bytes is not None
            else "Packed container: not supplied"
        ),
        "Allocated sections (informational; PT_LOAD totals remain authoritative):",
    ]
    for section in report.sections:
        runtime_region = "FLASH" if section.address < FLASH_END else "RAM"
        runtime_start = section.address
        runtime_end = runtime_start + section.size
        details = [
            f"runtime {runtime_region} "
            f"[0x{runtime_start:08x},0x{runtime_end:08x})"
        ]
        if section.load_address is not None:
            load_start = section.load_address
            load_end = load_start + section.size
            details.append(
                f"flash load [0x{load_start:08x},0x{load_end:08x})"
            )
        elif section.name == ".stack":
            details.append("reserved separately")
        else:
            details.append("no flash load (zero-filled)")
        lines.append(
            f"Section {section.name}: {section.size} bytes; " + "; ".join(details)
        )
    for segment in report.segments:
        lines.append(
            f"PT_LOAD[{segment.index}]: LMA 0x{segment.physical_address:08x} "
            f"+ {segment.file_size} file bytes; VMA "
            f"0x{segment.virtual_address:08x} + {segment.memory_size} memory bytes"
        )
    lines.extend(f"ERROR: {error}" for error in report.errors)
    if not report.errors:
        lines.append("Budget check passed")
    return "\n".join(lines)


def parse_stack_reservation(value: str) -> int:
    try:
        return int(value, 0)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(
            "stack reservation must be an integer byte count (decimal or 0x-prefixed)"
        ) from exc


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--elf", required=True, type=Path, help="linked ELF input")
    parser.add_argument("--raw", required=True, type=Path, help="raw flash payload")
    parser.add_argument(
        "--stack-reservation",
        required=True,
        type=parse_stack_reservation,
        help="required stack reservation in bytes (decimal or 0x-prefixed)",
    )
    parser.add_argument(
        "--packed",
        type=Path,
        help="optional packed container to report separately without limiting",
    )
    args = parser.parse_args(argv)

    try:
        report = analyze(args.elf, args.raw, args.stack_reservation, args.packed)
    except (BudgetError, OSError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1
    print(format_report(report, args.stack_reservation))
    return 1 if report.errors else 0


if __name__ == "__main__":
    sys.exit(main())
