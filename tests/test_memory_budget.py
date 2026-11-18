import os
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
ARM_BINUTILS_PREFIX = os.environ.get("ARM_BINUTILS_PREFIX")


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
        raw_data = bytearray(raw_size)
        for load_address, _, file_size, _ in segments:
            start = max(load_address, 0)
            end = min(load_address + file_size, raw_size)
            if start < end:
                raw_data[start:end] = b"\xa5" * (end - start)
        self.raw_path.write_bytes(raw_data)

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

    def test_raw_payload_must_match_elf_bytes(self):
        self.write_fixture([(0, 0, 4, 4)], 4)
        self.raw_path.write_bytes(b"\xa5\xa5\x00\xa5")
        report = analyze(self.elf_path, self.raw_path, STACK_RESERVATION)
        self.assertIn("byte at offset 0x2 does not match", "\n".join(report.errors))

    def test_raw_payload_must_start_at_origin_and_end_at_load_span(self):
        self.write_fixture([(0x100, 0x20000000, 4, 4)], 0x104)
        report = analyze(self.elf_path, self.raw_path, STACK_RESERVATION)
        self.assertIn("must start at application origin", "\n".join(report.errors))

        self.write_fixture([(0, 0, 1, 1)], 2)
        report = analyze(self.elf_path, self.raw_path, STACK_RESERVATION)
        self.assertIn("expected 1 bytes", "\n".join(report.errors))

    def test_empty_raw_payload_and_elf_without_flash_content_fail(self):
        self.write_fixture([(0, 0, 1, 1)], 0)
        report = analyze(self.elf_path, self.raw_path, STACK_RESERVATION)
        self.assertIn("raw payload is empty", "\n".join(report.errors))

        self.write_fixture([(0, 0, 0, 0)], 0)
        report = analyze(self.elf_path, self.raw_path, STACK_RESERVATION)
        self.assertIn("no file-backed flash load content", "\n".join(report.errors))

    def test_nonzero_file_backed_flash_xip_requires_matching_lma(self):
        self.write_fixture([(0, 1, 1, 1)], 1)
        report = analyze(self.elf_path, self.raw_path, STACK_RESERVATION)
        self.assertIn("flash XIP VMA", "\n".join(report.errors))

    def test_zero_filled_gap_between_loads_is_valid_padding(self):
        self.write_fixture([(0, 0, 1, 1), (4, 4, 1, 1)], 5)
        report = analyze(self.elf_path, self.raw_path, STACK_RESERVATION)
        self.assertEqual(report.errors, [])
        self.assertEqual(self.raw_path.read_bytes(), b"\xa5\0\0\0\xa5")

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
        self.write_fixture([(0, 0, 4, 4), (4, 2, 0, 1)], 4)
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
        self.write_fixture([(0, 0x20000000, 32, 64)], 32)
        report = analyze(self.elf_path, self.raw_path, STACK_RESERVATION)
        self.assertEqual(report.flash_load_bytes, 32)
        self.assertEqual(report.ram_static_bytes, 64)
        self.assertEqual(report.errors, [])

    def test_stack_reservation_must_match_project_policy(self):
        self.write_fixture([(0, 0, 1, 1)], 1)
        report = analyze(self.elf_path, self.raw_path, STACK_RESERVATION - 1)
        self.assertIn(
            "stack reservation must be exactly 2048 bytes",
            "\n".join(report.errors),
        )

    @unittest.skipUnless(ARM_BINUTILS_PREFIX, "set ARM_BINUTILS_PREFIX to run the real ARM image test")
    def test_real_arm_image_with_exact_fit_linked_stack(self):
        source = self.directory / "fixture.s"
        script = self.directory / "fixture.ld"
        obj = self.directory / "fixture.o"
        source.write_text(
            """
.section .text.entry, "ax"
.thumb
.byte 0x11, 0x22
.section .rodata.test, "a"
.byte 0x33
.section .data.test, "aw"
.word 0x12345678
.section .bss.test, "aw", %nobits
.space 0x37fc
""",
            encoding="ascii",
        )
        script.write_text(
            """
MEMORY {
  FLASH (rx) : ORIGIN = 0, LENGTH = 60K
  RAM (rw) : ORIGIN = 0x20000000, LENGTH = 16K
}
PHDRS { code PT_LOAD; data PT_LOAD; stack PT_LOAD; }
SECTIONS {
  .text : { *(.text.entry) } > FLASH :code
  .rodata ALIGN(16) : { *(.rodata.test) } > FLASH :code
  .data : { *(.data.test) } > RAM AT> FLASH :data
  .bss (NOLOAD) : { *(.bss.test) } > RAM :data
  .stack 0x20003800 (NOLOAD) : { . += 0x800; } > RAM :stack
}
""",
            encoding="ascii",
        )
        for tool, args in (
            ("as", ["-mcpu=cortex-m0", "-o", obj, source]),
            ("ld", ["-T", script, "-o", self.elf_path, obj]),
            ("objcopy", ["-O", "binary", self.elf_path, self.raw_path]),
        ):
            subprocess.run([ARM_BINUTILS_PREFIX + tool, *args], check=True)
        report = analyze(self.elf_path, self.raw_path, STACK_RESERVATION)
        self.assertEqual(report.errors, [])
        self.assertEqual(report.ram_static_bytes + STACK_RESERVATION, 0x4000)
        raw = self.raw_path.read_bytes()
        self.assertEqual(raw[:17], b"\x11\x22" + bytes(14) + b"\x33")
        self.raw_path.write_bytes(b"\xff" + raw[1:])
        self.check_fails("byte at offset 0x0 does not match")
        script.write_text(
            script.read_text(encoding="ascii").replace(". += 0x800;", ". += 0x7ff;"),
            encoding="ascii",
        )
        subprocess.run(
            [ARM_BINUTILS_PREFIX + "ld", "-T", script, "-o", self.elf_path, obj],
            check=True,
        )
        with self.assertRaisesRegex(BudgetError, r"linked \.stack must be .* exact reserved range"):
            analyze(self.elf_path, self.raw_path, STACK_RESERVATION)

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
