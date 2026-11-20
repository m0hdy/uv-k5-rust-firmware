import os
import re
import shutil
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import check_firmware_entry as entry_check  # noqa: E402


ELF_ENV = os.environ.get("FIRMWARE_ENTRY_ELF")


@unittest.skipUnless(ELF_ENV, "set FIRMWARE_ENTRY_ELF to test the linked entry")
class FirmwareEntryEmissionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.elf = Path(ELF_ENV).resolve()
        if not cls.elf.is_file():
            raise RuntimeError(f"linked firmware entry is missing: {cls.elf}")
        cls.probes = (
            ROOT / "target/firmware-entry-probes" / cls.elf.parents[2].name
        )
        shutil.rmtree(cls.probes, ignore_errors=True)
        cls.probes.mkdir(parents=True)
        cls.disassembly = cls.run_objdump(cls.elf, "-d", "-C", "--wide")
        sections = cls.run_objdump(cls.elf, "-h")
        match = re.search(
            r"(?m)^\s*\d+\s+\.text\s+([0-9a-f]+)\s+([0-9a-f]+)\s+"
            r"[0-9a-f]+\s+([0-9a-f]+)\b",
            sections,
        )
        if match is None:
            raise RuntimeError("linked firmware entry has no .text section")
        cls.text_size, cls.text_vma, cls.text_offset = (
            int(value, 16) for value in match.groups()
        )

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.probes, ignore_errors=True)

    @staticmethod
    def run_objdump(elf, *args):
        return subprocess.run(
            ["arm-none-eabi-objdump", *args, str(elf)],
            check=True,
            capture_output=True,
            text=True,
        ).stdout

    def validate(self, elf):
        entry_check.inspect_disassembly(
            self.run_objdump(elf, "-d", "-C", "--wide"),
            self.run_objdump(elf, "-t", "-C", "--wide"),
            self.run_objdump(elf, "-s", "-j", ".vector_table"),
        )

    def patch_halfword(self, name, address, replacement):
        self.assertTrue(self.text_vma <= address < self.text_vma + self.text_size)
        modified = bytearray(self.elf.read_bytes())
        offset = self.text_offset + address - self.text_vma
        modified[offset : offset + 2] = replacement.to_bytes(2, "little")
        probe = self.probes / name
        probe.write_bytes(modified)
        return probe

    def patch_halfwords(self, name, address, replacements):
        self.assertTrue(self.text_vma <= address < self.text_vma + self.text_size)
        modified = bytearray(self.elf.read_bytes())
        offset = self.text_offset + address - self.text_vma
        replacement = b"".join(
            halfword.to_bytes(2, "little") for halfword in replacements
        )
        modified[offset : offset + len(replacement)] = replacement
        probe = self.probes / name
        probe.write_bytes(modified)
        return probe

    def patch_words(self, name, replacements):
        modified = bytearray(self.elf.read_bytes())
        for address, replacement in replacements.items():
            self.assertTrue(
                self.text_vma <= address <= self.text_vma + self.text_size - 4
            )
            offset = self.text_offset + address - self.text_vma
            modified[offset : offset + 4] = replacement.to_bytes(4, "little")
        probe = self.probes / name
        probe.write_bytes(modified)
        return probe

    def read_halfword(self, address):
        self.assertTrue(self.text_vma <= address < self.text_vma + self.text_size)
        offset = self.text_offset + address - self.text_vma
        return int.from_bytes(self.elf.read_bytes()[offset : offset + 2], "little")

    def coordinator(self):
        functions = entry_check.function_bodies(self.disassembly)
        name = next(
            symbol for symbol in functions
            if "coordinate_with_welcome::<" in symbol
        )
        return name, functions[name]

    def test_actual_hardware_store_requires_mmio_provenance(self):
        functions = entry_check.function_bodies(self.disassembly)
        name = next(
            symbol for symbol in functions
            if "StartupIo>::systick" in symbol
        )
        body = functions[name]
        words = {
            int(address, 16): int(value, 16)
            for address, value in re.findall(
                r"(?m)^\s*([0-9a-fA-F]+):\s+[0-9a-fA-F]+\s+"
                r"\.word\s+0x([0-9a-fA-F]+)\b",
                body,
            )
        }
        literals = {}
        for address, mnemonic, operands in entry_check.instructions(body):
            load = re.fullmatch(
                r"r[0-7],\s*\[pc,\s*#(\d+)\].*",
                operands,
            )
            if mnemonic != "ldr" or load is None:
                continue
            literal = ((address + 4) & ~3) + int(load.group(1))
            value = words.get(literal)
            if value is not None and entry_check.is_mmio_address(value):
                literals[literal] = 0x20000000
        self.assertGreaterEqual(len(literals), 1)
        probe = self.patch_words("systick-non-mmio-stores.elf", literals)
        with self.assertRaisesRegex(
            entry_check.EntryError,
            "systick executor has no reachable hardware effect",
        ):
            self.validate(probe)

    def test_actual_coordinator_entry_self_loop_is_rejected(self):
        name, body = self.coordinator()
        address = entry_check.function_address(self.disassembly, name)
        rows = entry_check.instructions(body)
        self.assertEqual(rows[0][0], address)
        self.assertEqual(rows[0][1], "push")
        self.assertEqual(rows[1][0] - address, 2)
        probe = self.patch_halfword("coordinator-self-loop.elf", address, 0xE7FE)
        with self.assertRaisesRegex(
            entry_check.EntryError, "cannot reach|exactly 3 times"
        ):
            self.validate(probe)

    def test_actual_first_eeprom_error_cannot_continue(self):
        _, body = self.coordinator()
        rows = entry_check.instructions(body)
        reads = [
            index
            for index, (_, mnemonic, operands) in enumerate(rows)
            if mnemonic.startswith("bl")
            and "eeprom_read_transport::execute_read::<" in operands
            and "+0x" not in operands
        ]
        self.assertEqual(len(reads), 3)
        conditional = next(
            index
            for index in range(reads[0] + 1, reads[0] + 9)
            if entry_check.CONDITIONAL_BRANCH.fullmatch(rows[index][1])
        )
        success = entry_check.branch_target(rows[conditional][2])
        self.assertIsNotNone(success)
        branch_address, mnemonic, _ = rows[conditional + 1]
        self.assertIn(mnemonic, {"b", "b.n", "b.w"})
        displacement = success - (branch_address + 4)
        self.assertEqual(displacement & 1, 0)
        self.assertTrue(-2048 <= displacement <= 2046)
        encoding = 0xE000 | ((displacement >> 1) & 0x7FF)
        probe = self.patch_halfword(
            "eeprom-error-continues.elf", branch_address, encoding
        )
        with self.assertRaisesRegex(
            entry_check.EntryError, "first-error|success tag"
        ):
            self.validate(probe)

    def test_actual_eeprom_success_polarity_is_checked(self):
        _, body = self.coordinator()
        rows = entry_check.instructions(body)
        first_read = next(
            index
            for index, (_, mnemonic, operands) in enumerate(rows)
            if mnemonic.startswith("bl")
            and "eeprom_read_transport::execute_read::<" in operands
            and "+0x" not in operands
        )
        conditional = next(
            index
            for index in range(first_read + 1, first_read + 9)
            if entry_check.CONDITIONAL_BRANCH.fullmatch(rows[index][1])
        )
        address, mnemonic, _ = rows[conditional]
        self.assertIn(mnemonic, {"beq", "beq.n", "beq.w"})
        encoding = self.read_halfword(address)
        self.assertEqual(encoding & 0xFF00, 0xD000)
        probe = self.patch_halfword(
            "eeprom-success-polarity.elf", address, encoding ^ 0x0100
        )
        with self.assertRaisesRegex(
            entry_check.EntryError, "success tag does not select continuation"
        ):
            self.validate(probe)

    def test_actual_eeprom_predicate_flags_are_preserved(self):
        _, body = self.coordinator()
        rows = entry_check.instructions(body)
        first_read = next(
            index
            for index, (_, mnemonic, operands) in enumerate(rows)
            if mnemonic.startswith("bl")
            and "eeprom_read_transport::execute_read::<" in operands
            and "+0x" not in operands
        )
        conditional = next(
            index
            for index in range(first_read + 1, first_read + 9)
            if entry_check.CONDITIONAL_BRANCH.fullmatch(rows[index][1])
        )
        comparison = next(
            index
            for index in range(first_read + 1, conditional)
            if rows[index][1] == "cmp"
        )
        address, mnemonic, _ = rows[comparison + 1]
        self.assertIn(mnemonic, {"str", "strb", "strh"})
        probe = self.patch_halfword(
            "eeprom-predicate-flags.elf", address, 0x4280
        )
        with self.assertRaisesRegex(
            entry_check.EntryError, "predicate.*single comparison|flags are overwritten"
        ):
            self.validate(probe)

    def test_actual_eeprom_result_slot_is_reloaded(self):
        _, body = self.coordinator()
        rows = entry_check.instructions(body)
        first_read = next(
            index
            for index, (_, mnemonic, operands) in enumerate(rows)
            if mnemonic.startswith("bl")
            and "eeprom_read_transport::execute_read::<" in operands
            and "+0x" not in operands
        )
        load_index = first_read + 1
        address, mnemonic, operands = rows[load_index]
        self.assertEqual(mnemonic, "ldr")
        returned_slot = re.fullmatch(
            r"r0,\s*\[sp,\s*#(\d+)\].*", operands
        )
        self.assertIsNotNone(returned_slot)
        comparison = next(
            index
            for index in range(load_index + 1, first_read + 9)
            if rows[index][1] == "cmp"
        )
        unrelated_store = next(
            operands
            for _, mnemonic, operands in rows[comparison + 1 : first_read + 9]
            if mnemonic == "str" and "[sp," in operands
        )
        wrong_slot = int(re.search(r"#(\d+)", unrelated_store).group(1))
        self.assertNotEqual(wrong_slot, int(returned_slot.group(1)))
        self.assertEqual(wrong_slot % 4, 0)
        self.assertLessEqual(wrong_slot, 1020)
        encoding = 0x9800 | (wrong_slot // 4)
        probe = self.patch_halfword(
            "eeprom-wrong-result-slot.elf", address, encoding
        )
        with self.assertRaisesRegex(
            entry_check.EntryError, "returned result slot"
        ):
            self.validate(probe)

    def test_actual_eeprom_result_definition_cannot_be_bypassed(self):
        _, body = self.coordinator()
        rows = entry_check.instructions(body)
        first_read = next(
            index
            for index, (_, mnemonic, operands) in enumerate(rows)
            if mnemonic.startswith("bl")
            and "eeprom_read_transport::execute_read::<" in operands
            and "+0x" not in operands
        )
        definition = next(
            index
            for index in range(first_read - 1, -1, -1)
            if rows[index][1] == "add"
            and re.fullmatch(
                r"r0,\s*sp,\s*#\d+",
                rows[index][2].split("@")[0].strip(),
            )
        )
        branch_address, replaced_mnemonic, _ = rows[definition - 1]
        self.assertIn(replaced_mnemonic, {"str", "strb", "strh"})
        target = rows[definition + 1][0]
        displacement = target - (branch_address + 4)
        self.assertEqual(displacement & 1, 0)
        self.assertTrue(-2048 <= displacement <= 2046)
        encoding = 0xE000 | ((displacement >> 1) & 0x7FF)
        probe = self.patch_halfword(
            "eeprom-result-definition-bypass.elf", branch_address, encoding
        )
        with self.assertRaisesRegex(
            entry_check.EntryError, "dominating stack-relative definition"
        ):
            self.validate(probe)

    def test_actual_eeprom_result_definition_cannot_be_called_over(self):
        _, body = self.coordinator()
        rows = entry_check.instructions(body)
        first_read = next(
            index
            for index, (_, mnemonic, operands) in enumerate(rows)
            if mnemonic.startswith("bl")
            and "eeprom_read_transport::execute_read::<" in operands
            and "+0x" not in operands
        )
        definition = next(
            index
            for index in range(first_read - 1, -1, -1)
            if rows[index][1] == "add"
            and re.fullmatch(
                r"r0,\s*sp,\s*#\d+",
                rows[index][2].split("@")[0].strip(),
            )
        )
        address = rows[definition - 2][0]
        self.assertEqual(rows[definition - 1][0], address + 2)
        self.assertEqual(rows[definition][0], address + 4)
        target = rows[definition + 1][0]
        displacement = target - (address + 4)
        self.assertEqual(displacement & 1, 0)
        value = displacement & ((1 << 25) - 1)
        sign = (value >> 24) & 1
        i1, i2 = (value >> 23) & 1, (value >> 22) & 1
        j1, j2 = 1 ^ i1 ^ sign, 1 ^ i2 ^ sign
        first = 0xF000 | (sign << 10) | ((value >> 12) & 0x3FF)
        second = (
            0xD000
            | (j1 << 13)
            | (j2 << 11)
            | ((value >> 1) & 0x7FF)
        )
        probe = self.patch_halfwords(
            "eeprom-result-definition-call-bypass.elf",
            address,
            (first, second),
        )
        with self.assertRaisesRegex(
            entry_check.EntryError, "intra-function calls"
        ):
            self.validate(probe)


if __name__ == "__main__":
    unittest.main()
