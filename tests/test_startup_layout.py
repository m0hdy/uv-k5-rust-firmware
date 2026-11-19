import struct
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from check_startup_layout import (  # noqa: E402
    ELF32_HEADER,
    ELF32_PROGRAM_HEADER,
    ELF32_SECTION_HEADER,
    ELF32_SYMBOL,
    FLASH_END,
    INITIAL_SP,
    RAM_END,
    RAM_START,
    SHF_ALLOC,
    STACK_SIZE,
    STACK_START,
    StartupError,
    validate,
)

VECTOR_SIZE = 48 * 4
FLASH_FILE_SIZE = 0x100


def make_elf(
    *,
    entry=0xC1,
    initial_sp=INITIAL_SP,
    reset_vector=0xC1,
    reset_address=0xC1,
    stack_address=STACK_START,
    data_address=RAM_START,
    bss_address=None,
    data_size=4,
    bss_size=4,
    data_flags=3,
    bss_flags=3,
    text_address=0xC0,
    text_kind=1,
    text_file_offset_delta=VECTOR_SIZE,
    text_offset_in_ram_segment=None,
    ram_segment_address=RAM_START,
    ram_segment_file_size=None,
    ram_segment_memory_size=None,
    data_load_address=FLASH_FILE_SIZE,
    flash_memory_size=FLASH_FILE_SIZE,
    flash_segment_file_size=FLASH_FILE_SIZE,
    flash_segment_type=1,
    flash_load_address=0,
    vector_file_offset_delta=0,
    vector_address=0,
    vector_size=VECTOR_SIZE,
    vector_kind=1,
    vector_flags=2,
    symbol_overrides=None,
    duplicate_allocated_stack=False,
):
    header_size = ELF32_HEADER.size
    program_offset = header_size
    program_size = ELF32_PROGRAM_HEADER.size
    data_offset = header_size + 3 * program_size
    vector = bytearray(b"\x01\x01\x00\x00" * (VECTOR_SIZE // 4))
    struct.pack_into("<II", vector, 0, initial_sp, reset_vector)
    flash = bytearray(b"\0" * FLASH_FILE_SIZE)
    flash[:VECTOR_SIZE] = vector
    data_load_offset = data_offset + len(flash)
    if ram_segment_file_size is None:
        ram_segment_file_size = data_size
    if ram_segment_memory_size is None:
        ram_segment_memory_size = data_size + bss_size
    initialized_data = b"\xa5" * max(data_size, ram_segment_file_size)
    stack_offset = data_load_offset + len(initialized_data)
    if text_offset_in_ram_segment is None:
        text_file_offset = data_offset + text_file_offset_delta
    else:
        text_file_offset = data_load_offset + text_offset_in_ram_segment
    if bss_address is None:
        bss_address = RAM_START + data_size
    program_headers = (
        ELF32_PROGRAM_HEADER.pack(
            flash_segment_type,
            data_offset,
            0,
            flash_load_address,
            flash_segment_file_size,
            flash_memory_size,
            5,
            4,
        )
        + ELF32_PROGRAM_HEADER.pack(
            1, data_load_offset, ram_segment_address, data_load_address, ram_segment_file_size,
            ram_segment_memory_size, 6, 4,
        )
        + ELF32_PROGRAM_HEADER.pack(
            1, stack_offset, STACK_START, STACK_START, 0, STACK_SIZE, 6, 4
        )
    )
    payload = bytearray(bytes(flash) + initialized_data)
    if vector_file_offset_delta:
        vector_end = vector_file_offset_delta + len(vector)
        if len(payload) < vector_end:
            payload.extend(bytes(vector_end - len(payload)))
        payload[vector_file_offset_delta:vector_end] = vector
    section_names = b"\0.vector_table\0.text\0.data\0.bss\0.stack\0.shstrtab\0.strtab\0.symtab\0"
    symbol_names = b"\0"
    symbol_name_offsets = {}
    symbol_values = {
        "Reset_Handler": reset_address,
        "__vector_table_start": 0,
        "__vector_table_end": VECTOR_SIZE,
        "__stack_start": STACK_START,
        "__stack_end": RAM_END,
        "__stack_top": RAM_END,
        "__data_load": data_load_address,
        "__data_start": data_address,
        "__data_end": data_address + data_size,
        "__bss_start": bss_address,
        "__bss_end": bss_address + bss_size,
    }
    if symbol_overrides:
        symbol_values.update(symbol_overrides)
    symbol_table_entries = [ELF32_SYMBOL.pack(0, 0, 0, 0, 0, 0)]
    for name, value in symbol_values.items():
        symbol_name_offsets[name] = len(symbol_names)
        symbol_names += name.encode() + b"\0"
        symbol_table_entries.append(
            ELF32_SYMBOL.pack(
                symbol_name_offsets[name],
                value,
                42 if name == "Reset_Handler" else 0,
                0x12 if name == "Reset_Handler" else 0,
                0,
                2 if name == "Reset_Handler" else 0xFFF1,
            )
        )
    names_offset = data_offset + len(payload)
    symbols_offset = names_offset + len(section_names) + len(symbol_names)
    symbol_table = b"".join(symbol_table_entries)
    section_offset = symbols_offset + len(symbol_table)
    section_name_offsets = {
        name: section_names.index(name.encode()) for name in (
            ".vector_table", ".text", ".data", ".bss", ".stack",
            ".shstrtab", ".strtab", ".symtab",
        )
    }
    sections = [
        bytes(ELF32_SECTION_HEADER.size),
        ELF32_SECTION_HEADER.pack(
            section_name_offsets[".vector_table"], vector_kind, vector_flags, vector_address,
            data_offset + vector_file_offset_delta,
            vector_size, 0, 0, 4, 0,
        ),
        ELF32_SECTION_HEADER.pack(
            section_name_offsets[".text"], text_kind, 6, text_address, text_file_offset,
            0x40, 0, 0, 4, 0,
        ),
        ELF32_SECTION_HEADER.pack(
            section_name_offsets[".data"], 1, data_flags, data_address, data_load_offset,
            data_size, 0, 0, 4, 0,
        ),
        ELF32_SECTION_HEADER.pack(
            section_name_offsets[".bss"], 8, bss_flags, bss_address, stack_offset,
            bss_size, 0, 0, 4, 0,
        ),
        ELF32_SECTION_HEADER.pack(
            section_name_offsets[".stack"], 8, 3, stack_address, stack_offset,
            STACK_SIZE, 0, 0, 8, 0,
        ),
        ELF32_SECTION_HEADER.pack(
            section_name_offsets[".shstrtab"], 3, 0, 0, names_offset,
            len(section_names), 0, 0, 1, 0,
        ),
        ELF32_SECTION_HEADER.pack(
            section_name_offsets[".strtab"], 3, 0, 0, names_offset + len(section_names),
            len(symbol_names), 0, 0, 1, 0,
        ),
        ELF32_SECTION_HEADER.pack(
            section_name_offsets[".symtab"], 2, 0, 0, symbols_offset,
            len(symbol_table), 7, 1, 4, ELF32_SYMBOL.size,
        ),
    ]
    if duplicate_allocated_stack:
        invalid_stack = ELF32_SECTION_HEADER.pack(
            section_name_offsets[".stack"], 8, 3, 0x40000000, stack_offset,
            STACK_SIZE, 0, 0, 8, 0,
        )
        valid_stack = sections.pop(5)
        sections.insert(5, invalid_stack)
        sections.append(valid_stack)
    ident = b"\x7fELF" + bytes([1, 1, 1]) + bytes(9)
    header = ELF32_HEADER.pack(
        ident, 2, 40, 1, entry, program_offset, section_offset, 0,
        header_size, program_size, 3, ELF32_SECTION_HEADER.size, len(sections), 6,
    )
    return (
        header + program_headers + bytes(payload) + section_names + symbol_names
        + symbol_table + b"".join(sections)
    )


def make_map(stack_end=RAM_END):
    return "\n".join(
        (
            "       0        0        0     1 __vector_table_start = .",
            "      c0       c0        0     1 __vector_table_end = .",
            f"{STACK_START:08x} {STACK_START:08x} 0 1 __stack_start = .",
            f"{stack_end:08x} {stack_end:08x} 0 1 __stack_end = .",
            f"{RAM_END:08x} {RAM_END:08x} 0 1 __stack_top = .",
        )
    )


class StartupLayoutTests(unittest.TestCase):
    def setUp(self):
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.directory = Path(self.temporary_directory.name)
        self.elf_path = self.directory / "fixture.elf"
        self.map_path = self.directory / "fixture.map"

    def tearDown(self):
        self.temporary_directory.cleanup()

    def check_fails(self, expected, **elf_options):
        self.elf_path.write_bytes(make_elf(**elf_options))
        self.map_path.write_text(make_map(), encoding="utf-8")
        with self.assertRaisesRegex(StartupError, expected):
            validate(self.elf_path, self.map_path)

    def test_valid_startup_layout_passes(self):
        self.elf_path.write_bytes(make_elf())
        self.map_path.write_text(make_map(), encoding="utf-8")
        validate(self.elf_path, self.map_path)

    def test_data_and_bss_boundary_adjacency_passes(self):
        self.elf_path.write_bytes(make_elf(data_size=8, bss_address=RAM_START + 8))
        self.map_path.write_text(make_map(), encoding="utf-8")
        validate(self.elf_path, self.map_path)

    def test_overlapping_text_and_vector_table_are_rejected(self):
        self.check_fails(
            r"allocated section ranges overlap: .*section \.text .*section \.vector_table ",
            entry=1,
            reset_vector=1,
            reset_address=1,
            text_address=0,
            text_file_offset_delta=0,
        )

    def test_empty_data_and_bss_sections_pass(self):
        self.elf_path.write_bytes(make_elf(data_size=0, bss_size=0))
        self.map_path.write_text(make_map(), encoding="utf-8")
        validate(self.elf_path, self.map_path)

    def test_duplicate_allocated_section_names_are_rejected(self):
        self.check_fails(
            "duplicate allocated section name \\.stack",
            duplicate_allocated_stack=True,
        )

    def test_vector_table_symbols_must_match_elf_layout(self):
        self.check_fails(
            "ELF symbol __vector_table_end has an invalid value",
            symbol_overrides={"__vector_table_end": VECTOR_SIZE - 4},
        )

    def test_vector_table_must_cover_the_first_48_flash_words(self):
        for options in ({"vector_address": 4}, {"vector_size": VECTOR_SIZE - 4}):
            with self.subTest(**options):
                self.check_fails(
                    "vector table must occupy the first 48 words of application flash",
                    **options,
                )

    def test_vector_table_must_be_allocated_progbits(self):
        for options in ({"vector_kind": 8}, {"vector_flags": 0}):
            with self.subTest(**options):
                self.check_fails(
                    "vector table must be an allocated PROGBITS section",
                    **options,
                )

    def test_data_load_must_cover_all_initialized_bytes(self):
        self.check_fails(
            "__data_load does not map the file-backed .data section",
            ram_segment_file_size=2,
        )

    def test_initialization_symbols_must_match_section_boundaries(self):
        self.check_fails(
            "__bss_end does not match its section boundary",
            symbol_overrides={"__bss_end": RAM_START + 3},
        )

    def test_initialization_symbols_must_be_word_aligned(self):
        self.check_fails(
            "__data_load must be 4-byte aligned",
            symbol_overrides={"__data_load": FLASH_FILE_SIZE + 1},
        )

    def test_ram_initialization_boundaries_must_be_word_aligned(self):
        cases = (
            ("__data_start", {
                "data_address": RAM_START + 1,
                "ram_segment_address": RAM_START + 1,
                "bss_address": RAM_START + 8,
            }),
            ("__data_end", {"data_size": 5, "bss_address": RAM_START + 8}),
            ("__bss_start", {"bss_address": RAM_START + 5}),
            ("__bss_end", {"bss_size": 5}),
        )
        for symbol, options in cases:
            with self.subTest(symbol=symbol):
                self.check_fails(f"{symbol} must be 4-byte aligned", **options)

    def test_entry_must_be_reset_handler(self):
        self.check_fails("entry point is not Reset_Handler", entry=0xC3)

    def test_entry_must_select_thumb_even_when_reset_vector_does(self):
        self.check_fails(
            "ELF entry point must select Thumb state",
            entry=0xC0,
            reset_address=0xC0,
        )

    def test_reset_vector_must_be_thumb_reset_handler(self):
        self.check_fails("reset vector is not the Thumb", reset_vector=0xC0)

    def test_reset_handler_must_be_in_executable_text(self):
        self.check_fails(
            "Reset_Handler must reside",
            entry=0x101,
            reset_vector=0x101,
            reset_address=0x101,
        )

    def test_reset_handler_requires_file_backed_text(self):
        self.check_fails("file-backed executable", text_kind=8)

    def test_reset_handler_cannot_extend_past_text(self):
        self.check_fails(
            "file-backed executable",
            entry=0xF1,
            reset_vector=0xF1,
            reset_address=0xF1,
        )

    def test_reset_handler_must_execute_from_flash(self):
        self.check_fails(
            "Reset_Handler must execute from application flash",
            entry=RAM_START + 0x101,
            reset_vector=RAM_START + 0x101,
            reset_address=RAM_START + 0x101,
            text_address=RAM_START + 0x100,
            text_offset_in_ram_segment=0x100,
            ram_segment_file_size=0x140,
            ram_segment_memory_size=0x140,
        )

    def test_initial_stack_pointer_must_match_reference(self):
        self.check_fails("initial stack pointer", initial_sp=RAM_END)

    def test_stack_reservation_must_fit_the_ram_top(self):
        self.check_fails("stack section must reserve", stack_address=STACK_START - 4)

    def test_data_must_reside_in_static_ram(self):
        self.check_fails(
            ".data must reside in static RAM",
            data_address=0,
        )

    def test_bss_must_reside_in_static_ram(self):
        self.check_fails(
            ".bss must reside in static RAM",
            bss_address=0,
        )

    def test_data_and_bss_must_not_overlap(self):
        self.check_fails(
            ".data and .bss ranges must not overlap",
            bss_address=RAM_START + 2,
        )

    def test_ram_load_segment_must_not_overlap_reserved_stack(self):
        self.check_fails(
            "RAM execution range overlaps the reserved stack",
            ram_segment_memory_size=0x3804,
        )

    def test_ram_load_segment_must_not_initialize_reserved_stack(self):
        self.check_fails(
            "initializes bytes in the reserved stack",
            ram_segment_file_size=0x3804,
            ram_segment_memory_size=0x3804,
        )

    def test_initialized_data_must_be_writable(self):
        self.check_fails(
            ".data must be writable",
            data_size=1,
            data_flags=SHF_ALLOC,
        )

    def test_reset_handler_must_fit_application_flash(self):
        self.check_fails(
            "Reset_Handler must execute from application flash",
            entry=FLASH_END + 1,
            reset_vector=FLASH_END + 1,
            reset_address=FLASH_END + 1,
            text_address=FLASH_END,
        )

    def test_load_segment_must_fit_application_flash(self):
        self.check_fails(
            "execution range is outside flash and RAM",
            flash_memory_size=FLASH_END + 1,
        )

    def test_flash_load_segment_vma_and_lma_must_match(self):
        self.check_fails(
            "flash VMA and LMA do not match",
            flash_load_address=FLASH_FILE_SIZE * 2,
        )

    def test_file_backed_load_must_fit_application_flash(self):
        self.check_fails(
            "load range is outside application flash",
            data_load_address=FLASH_END - 4,
            data_size=8,
        )

    def test_flash_load_cannot_overlap_another_flash_execution_range(self):
        self.check_fails(
            "flash load range overlaps .* flash execution range",
            flash_memory_size=FLASH_FILE_SIZE + 4,
        )

    def test_vector_table_must_be_covered_by_a_load_segment(self):
        self.check_fails(
            "allocated section .vector_table is not covered by a file-backed PT_LOAD",
            flash_segment_type=0,
        )

    def test_text_must_be_covered_by_a_load_segment(self):
        self.check_fails(
            "allocated section .text is not covered by a file-backed PT_LOAD",
            flash_segment_file_size=VECTOR_SIZE,
            flash_memory_size=VECTOR_SIZE,
        )

    def test_allocated_section_file_offset_must_match_load_segment(self):
        self.check_fails(
            "allocated section .vector_table is not covered by a file-backed PT_LOAD",
            vector_file_offset_delta=FLASH_FILE_SIZE + 4,
        )

    def test_flash_load_ranges_must_not_overlap(self):
        self.check_fails(
            "PT_LOAD flash load/file ranges overlap",
            data_load_address=0,
        )

    def test_map_stack_end_entry_is_required(self):
        self.elf_path.write_bytes(make_elf())
        self.map_path.write_text(
            "\n".join(
                line for line in make_map().splitlines() if "__stack_end" not in line
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(StartupError, "linker map is missing __stack_end"):
            validate(self.elf_path, self.map_path)

    def test_map_direct_stack_end_must_match_elf_boundary(self):
        self.elf_path.write_bytes(make_elf())
        self.map_path.write_text(make_map(stack_end=STACK_START), encoding="utf-8")
        with self.assertRaisesRegex(
            StartupError, "linker map __stack_end address does not match the ELF"
        ):
            validate(self.elf_path, self.map_path)

    def test_map_stack_top_expression_uses_resolved_elf_symbol(self):
        self.elf_path.write_bytes(make_elf())
        self.map_path.write_text(
            make_map().replace(
                "20004000 20004000 0 1 __stack_top = .",
                "20004000 20003800 0 1 __stack_top = ORIGIN(STACK)+LENGTH(STACK)",
            ),
            encoding="utf-8",
        )
        validate(self.elf_path, self.map_path)

    def test_incorrect_elf_stack_top_is_rejected_despite_map_location_counter(self):
        self.elf_path.write_bytes(
            make_elf(symbol_overrides={"__stack_top": STACK_START})
        )
        self.map_path.write_text(
            make_map().replace(
                "20004000 20004000 0 1 __stack_top = .",
                "20004000 20003800 0 1 __stack_top = ORIGIN(STACK)+LENGTH(STACK)",
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            StartupError, "ELF symbol __stack_top has an invalid value"
        ):
            validate(self.elf_path, self.map_path)


if __name__ == "__main__":
    unittest.main()
