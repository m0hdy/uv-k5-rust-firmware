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
    STACK_SIZE,
    STACK_START,
    StartupError,
    validate,
)

ROOT = Path(__file__).resolve().parents[1]
VECTOR_SIZE = 48 * 4
FLASH_FILE_SIZE = 0x100


def make_elf(
    *,
    entry=0xC1,
    initial_sp=INITIAL_SP,
    reset_vector=0xC1,
    stack_address=STACK_START,
    text_address=0xC0,
    flash_memory_size=FLASH_FILE_SIZE,
):
    header_size = ELF32_HEADER.size
    program_offset = header_size
    program_size = ELF32_PROGRAM_HEADER.size
    data_offset = header_size + 2 * program_size
    vector = bytearray(b"\x01\x01\x00\x00" * (VECTOR_SIZE // 4))
    struct.pack_into("<II", vector, 0, initial_sp, reset_vector)
    flash = bytearray(b"\0" * FLASH_FILE_SIZE)
    flash[:VECTOR_SIZE] = vector
    ram_offset = data_offset + len(flash)
    program_headers = (
        ELF32_PROGRAM_HEADER.pack(
            1, data_offset, 0, 0, len(flash), flash_memory_size, 5, 4
        )
        + ELF32_PROGRAM_HEADER.pack(
            1, ram_offset, RAM_START, RAM_START, 0, RAM_END - RAM_START, 6, 4
        )
    )
    payload = bytes(flash)
    section_names = b"\0.vector_table\0.text\0.data\0.bss\0.stack\0.shstrtab\0.strtab\0.symtab\0"
    symbol_names = b"\0Reset_Handler\0"
    names_offset = data_offset + len(payload)
    symbols_offset = names_offset + len(section_names) + len(symbol_names)
    symbol_table = ELF32_SYMBOL.pack(0, 0, 0, 0, 0, 0) + ELF32_SYMBOL.pack(
        1, 0xC1, 42, 0x12, 0, 2
    )
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
            section_name_offsets[".vector_table"], 1, 2, 0, data_offset,
            VECTOR_SIZE, 0, 0, 4, 0,
        ),
        ELF32_SECTION_HEADER.pack(
            section_name_offsets[".text"], 1, 6, text_address, data_offset + 0xC0,
            0x40, 0, 0, 4, 0,
        ),
        ELF32_SECTION_HEADER.pack(
            section_name_offsets[".data"], 1, 3, RAM_START, ram_offset, 0, 0, 0, 4, 0,
        ),
        ELF32_SECTION_HEADER.pack(
            section_name_offsets[".bss"], 8, 3, RAM_START, ram_offset, 0, 0, 0, 4, 0,
        ),
        ELF32_SECTION_HEADER.pack(
            section_name_offsets[".stack"], 8, 3, stack_address, ram_offset,
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
    ident = b"\x7fELF" + bytes([1, 1, 1]) + bytes(9)
    header = ELF32_HEADER.pack(
        ident, 2, 40, 1, entry, program_offset, section_offset, 0,
        header_size, program_size, 2, ELF32_SECTION_HEADER.size, len(sections), 6,
    )
    return (
        header + program_headers + payload + section_names + symbol_names
        + symbol_table + b"".join(sections)
    )


def make_map(stack_end=RAM_END):
    return "\n".join(
        (
            "       0        0        0     1 __vector_table_start = .",
            "      c0       c0        0     1 __vector_table_end = .",
            f"{STACK_START:08x} {STACK_START:08x} 0 1 __stack_start = .",
            f"{stack_end:08x} {stack_end:08x} 0 1 __stack_end = .",
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

    def test_entry_must_be_reset_handler(self):
        self.check_fails("entry point is not Reset_Handler", entry=0xC3)

    def test_reset_vector_must_be_thumb_reset_handler(self):
        self.check_fails("reset vector is not the Thumb", reset_vector=0xC0)

    def test_initial_stack_pointer_must_match_reference(self):
        self.check_fails("initial stack pointer", initial_sp=RAM_END)

    def test_stack_reservation_must_fit_the_ram_top(self):
        self.check_fails("stack section must reserve", stack_address=STACK_START - 4)

    def test_allocated_text_must_fit_application_flash(self):
        self.check_fails("allocated section .text", text_address=FLASH_END)

    def test_load_segment_must_fit_application_flash(self):
        self.check_fails("execution range is outside flash and RAM",
                         flash_memory_size=FLASH_END + 1)

    def test_map_stack_end_must_match_ram_end(self):
        self.elf_path.write_bytes(make_elf())
        self.map_path.write_text(make_map(stack_end=RAM_END - 4), encoding="utf-8")
        with self.assertRaisesRegex(StartupError, "invalid or missing __stack_end"):
            validate(self.elf_path, self.map_path)

if __name__ == "__main__":
    unittest.main()
