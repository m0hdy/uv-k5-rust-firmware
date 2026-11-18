import struct
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from check_memory_budget import (  # noqa: E402
    BudgetError,
    analyze,
    format_report,
    parse_elf,
)

ELF_HEADER = struct.Struct("<16sHHIIIIIHHHHHH")
PROGRAM_HEADER = struct.Struct("<IIIIIIII")
STACK_RESERVATION = 0x800
ROOT = Path(__file__).resolve().parents[1]
CHECKER = ROOT / "scripts" / "check_memory_budget.py"


def make_elf(segments, debug_bytes=b""):
    header_size = ELF_HEADER.size
    program_header_size = PROGRAM_HEADER.size
    program_header_offset = header_size
    data_offset = header_size + len(segments) * program_header_size
    program_headers = []
    payload_parts = []
    current_offset = data_offset
    for load_address, execution_address, file_size, memory_size in segments:
        program_headers.append(
            PROGRAM_HEADER.pack(
                1,
                current_offset,
                execution_address,
                load_address,
                file_size,
                memory_size,
                4,
                4,
            )
        )
        payload_parts.append(b"\xa5" * file_size)
        current_offset += file_size

    section_strings = b"\0.debug_info\0.shstrtab\0"
    debug_offset = current_offset
    strings_offset = debug_offset + len(debug_bytes)
    section_offset = strings_offset + len(section_strings)
    sections = [
        bytes(40),
        struct.pack("<IIIIIIIIII", 1, 1, 0, 0, debug_offset, len(debug_bytes), 0, 0, 1, 0),
        struct.pack("<IIIIIIIIII", 11, 3, 0, 0, strings_offset, len(section_strings), 0, 0, 1, 0),
    ]
    ident = b"\x7fELF" + bytes([1, 1, 1]) + bytes(9)
    header = ELF_HEADER.pack(
        ident,
        2,
        40,
        1,
        0,
        program_header_offset,
        section_offset,
        0,
        header_size,
        program_header_size,
        len(program_headers),
        40,
        len(sections),
        2,
    )
    return (
        header
        + b"".join(program_headers)
        + b"".join(payload_parts)
        + debug_bytes
        + section_strings
        + b"".join(sections)
    )


class BudgetCheckerTests(unittest.TestCase):
    def setUp(self):
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.directory = Path(self.temporary_directory.name)
        self.elf_path = self.directory / "fixture.elf"
        self.raw_path = self.directory / "fixture.bin"

    def tearDown(self):
        self.temporary_directory.cleanup()

    def write_fixture(self, segments, raw_size, debug_bytes=b""):
        self.elf_path.write_bytes(make_elf(segments, debug_bytes))
        self.raw_path.write_bytes(b"\0" * raw_size)

    def check_fails(self, expected):
        report = analyze(self.elf_path, self.raw_path, STACK_RESERVATION)
        self.assertIn(expected, "\n".join(report.errors))

    def test_allowed_flash_and_ram_boundaries_pass(self):
        self.write_fixture(
            [
                (0, 0, 0xE000, 0xE000),
                (
                    0xE000,
                    0x20000000,
                    0x1000,
                    0x4000 - STACK_RESERVATION,
                ),
            ],
            0xF000,
        )
        report = analyze(self.elf_path, self.raw_path, STACK_RESERVATION)
        self.assertEqual(report.errors, [])
        self.assertEqual(report.flash_load_bytes, 0xF000)
        self.assertEqual(report.ram_static_bytes, 0x4000 - STACK_RESERVATION)
        summary = format_report(report, STACK_RESERVATION)
        self.assertIn("Flash load bytes: 61440 / 61440 (0 remaining)", summary)
        self.assertIn("RAM use plus stack: 16384 / 16384 (0 remaining)", summary)

    def test_raw_payload_one_byte_over_fails(self):
        self.write_fixture([(0, 0, 1, 1)], 0xF001)
        report = analyze(self.elf_path, self.raw_path, STACK_RESERVATION)
        self.assertIn("maximum is 61440 bytes", "\n".join(report.errors))
        self.assertIn(
            "Raw payload: 61441 / 61440 (-1 remaining)",
            format_report(report, STACK_RESERVATION),
        )

    def test_out_of_region_load_address_fails(self):
        self.write_fixture([(0xF000, 0xF000, 1, 1)], 0xF001)
        self.check_fails("outside flash")

    def test_ram_plus_stack_overflow_fails(self):
        self.write_fixture([(0, 0x20000000, 1, 0x3801)], 1)
        report = analyze(self.elf_path, self.raw_path, STACK_RESERVATION)
        self.assertTrue(any("overlaps the reserved stack" in e for e in report.errors))
        self.assertTrue(any("RAM use plus stack is 16385 bytes" in e for e in report.errors))

    def test_overlapping_load_ranges_fail(self):
        self.write_fixture([(0, 0, 4, 4), (2, 4, 4, 4)], 6)
        self.check_fails("Flash load/file ranges overlap")

    def test_overlapping_runtime_ranges_fail(self):
        self.write_fixture(
            [(0, 0x20000000, 4, 4), (4, 0x20000002, 4, 4)],
            8,
        )
        self.check_fails("RAM execution/memory ranges overlap")

    def test_flash_load_and_another_execution_range_overlap(self):
        self.write_fixture([(0, 16, 4, 4), (4, 2, 1, 1)], 8)
        self.check_fails("overlaps PT_LOAD[1] flash execution/memory range")

    def test_debug_only_file_growth_does_not_change_budget(self):
        segments = [(0, 0, 16, 16)]
        self.write_fixture(segments, 16)
        before = analyze(self.elf_path, self.raw_path, STACK_RESERVATION)
        self.elf_path.write_bytes(make_elf(segments, b"debug symbols" * 1000))
        after = analyze(self.elf_path, self.raw_path, STACK_RESERVATION)
        self.assertEqual(before.flash_load_bytes, after.flash_load_bytes)
        self.assertEqual(before.ram_static_bytes, after.ram_static_bytes)
        self.assertEqual(before.raw_bytes, after.raw_bytes)
        self.assertEqual(after.errors, [])

    def test_initialized_ram_uses_flash_lma_and_ram_vma(self):
        self.write_fixture([(0x100, 0x20000000, 32, 64)], 0x120)
        report = analyze(self.elf_path, self.raw_path, STACK_RESERVATION)
        self.assertEqual(report.flash_load_bytes, 32)
        self.assertEqual(report.ram_static_bytes, 64)
        self.assertEqual(report.errors, [])

    def test_truncated_elf_and_segment_data_fail(self):
        self.elf_path.write_bytes(b"\x7fELF")
        with self.assertRaisesRegex(BudgetError, "truncated"):
            parse_elf(self.elf_path)
        self.elf_path.write_bytes(make_elf([(0, 0, 16, 16)]))
        elf = bytearray(self.elf_path.read_bytes())
        program_header = list(PROGRAM_HEADER.unpack_from(elf, ELF_HEADER.size))
        program_header[1] = len(elf) - 1
        PROGRAM_HEADER.pack_into(elf, ELF_HEADER.size, *program_header)
        self.elf_path.write_bytes(elf)
        with self.assertRaisesRegex(BudgetError, "truncated"):
            parse_elf(self.elf_path)

    def test_malformed_elf_and_missing_load_segments_fail(self):
        self.elf_path.write_bytes(b"not elf")
        with self.assertRaisesRegex(BudgetError, "truncated"):
            parse_elf(self.elf_path)
        self.elf_path.write_bytes(make_elf([]))
        with self.assertRaisesRegex(BudgetError, "no program headers"):
            parse_elf(self.elf_path)

    def test_packed_length_is_reported_without_raw_limit(self):
        self.write_fixture([(0, 0, 1, 1)], 1)
        packed_path = self.directory / "fixture.packed"
        packed_path.write_bytes(b"\0" * 70000)
        report = analyze(self.elf_path, self.raw_path, STACK_RESERVATION, packed_path)
        self.assertEqual(report.packed_bytes, 70000)
        self.assertEqual(report.errors, [])
        self.assertIn("Packed container: 70000 bytes", format_report(report, STACK_RESERVATION))

    def test_stack_reservation_is_required_and_malformed_input_fails(self):
        self.write_fixture([(0, 0, 1, 1)], 1)
        missing = subprocess.run(
            [sys.executable, str(CHECKER), "--elf", str(self.elf_path), "--raw", str(self.raw_path)],
            capture_output=True,
            text=True,
        )
        malformed = subprocess.run(
            [
                sys.executable,
                str(CHECKER),
                "--elf",
                str(self.elf_path),
                "--raw",
                str(self.raw_path),
                "--stack-reservation",
                "unknown",
            ],
            capture_output=True,
            text=True,
        )
        self.assertNotEqual(missing.returncode, 0)
        self.assertIn("--stack-reservation", missing.stderr)
        self.assertNotEqual(malformed.returncode, 0)
        self.assertIn("stack reservation must be an integer", malformed.stderr)


if __name__ == "__main__":
    unittest.main()
