"""Inspect the real, never-executed witness; host-only runs may lack the ELF/tools.

The canonical CI witness build precedes the Python suite, so its artifact and
ARM binutils are expected there. A host-only run skips when either is absent.
"""

import os
import re
import shutil
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import check_startup_witness as witness  # noqa: E402


ELF = ROOT / "target/startup-witness/thumbv6m-none-eabi/release/startup-adapter-witness"


class StartupWitnessEmissionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        prefix = os.environ.get("ARM_BINUTILS_PREFIX", "arm-none-eabi-")
        missing = []
        if not ELF.is_file():
            missing.append(f"linked startup witness {ELF}")
        missing.extend(
            prefix + tool for tool in ("objdump", "nm")
            if shutil.which(prefix + tool) is None
        )
        if missing:
            message = "required for linked witness integration: " + ", ".join(missing)
            if os.environ.get("CI"):
                raise RuntimeError(message)
            raise unittest.SkipTest(message)
        cls.disassembly = witness.run_tool("objdump", "-d", "-C", str(ELF))
        cls.run_tool = staticmethod(witness.run_tool)
        # A negative must not pass merely because the unmodified ELF is invalid.
        witness.validate(ELF)

    def validate_emission(self, disassembly, text_contents=None):
        def run_tool(name, *args):
            if name == "objdump" and args[:2] == ("-d", "-C"):
                return disassembly
            if text_contents is not None and name == "objdump" and args[:3] == ("-s", "-j", ".text"):
                return text_contents
            return self.run_tool(name, *args)

        with patch.object(witness, "run_tool", side_effect=run_tool):
            return witness.validate(ELF)

    def entry(self, fragment, suffix=False):
        return witness.function_entry(self.disassembly, fragment, suffix)

    def replace_at(self, disassembly, address, expected, replacement):
        pattern = rf"(?m)^{address:x}:\s+.*$"
        matches = list(re.finditer(pattern, disassembly))
        self.assertEqual(len(matches), 1, f"expected one instruction at {address:x}")
        old = matches[0].group()
        self.assertRegex(old, expected)
        return disassembly[:matches[0].start()] + replacement + disassembly[matches[0].end():]

    def replace_instruction(self, fragment, offset, expected, encoding, instruction, suffix=False):
        address, _ = self.entry(fragment, suffix)
        address += offset
        return self.replace_at(
            self.disassembly, address, expected, f"{address:x}:\t{encoding}\t{instruction}"
        )

    def bypass_call(self, address, expected_before, expected_call):
        pattern = rf"(?m)^{address:x}:\s+.*$"
        calls = list(re.finditer(pattern, self.disassembly))
        self.assertEqual(len(calls), 1)
        self.assertRegex(calls[0].group(), expected_call)
        before = address - 2
        return self.replace_at(
            self.disassembly, before, expected_before,
            f"{before:x}:\te001\tb.n {address + 4:x}",
        )

    def redirect_call(
        self, caller, offset, callee, suffix=False, callee_suffix=False, replacement_callee=None
    ):
        caller_address, _ = self.entry(caller, suffix)
        callee_address, callee_name = self.entry(callee, callee_suffix)
        address = caller_address + offset
        if replacement_callee is None:
            target_address, target_name = callee_address + 2, callee_name + "+0x2"
        else:
            target_address, target_name = self.entry(replacement_callee, True)
        # Encode a genuine Thumb BL, not merely a changed symbol annotation.
        displacement = target_address - (address + 4)
        self.assertEqual(displacement & 1, 0)
        self.assertTrue(-(1 << 24) <= displacement < (1 << 24))
        value = displacement & ((1 << 25) - 1)
        s, i1, i2 = (value >> 24) & 1, (value >> 23) & 1, (value >> 22) & 1
        j1, j2 = 1 ^ i1 ^ s, 1 ^ i2 ^ s
        first = 0xF000 | (s << 10) | ((value >> 12) & 0x3FF)
        second = 0xD000 | (j1 << 13) | (j2 << 11) | ((value >> 1) & 0x7FF)
        return self.replace_at(
            self.disassembly, address,
            rf"\bbl\s+{callee_address:x}\s+<{re.escape(callee_name)}>\s*$",
            f"{address:x}:\t{first:04x} {second:04x}\tbl\t{target_address:x} <{target_name}>",
        )

    def test_unmodified_linked_elf_passes_full_validator(self):
        flash, ram = self.validate_emission(self.disassembly)
        self.assertGreater(flash, 0)
        self.assertGreaterEqual(ram, 0)

    def test_calls_must_target_real_numeric_entries(self):
        cases = (
            ("StartupIo>::bk1080_shutdown", 0x0C, "bk1080_startup_adapter::shutdown_mmio::<", True, False),
            ("bk1080_startup_adapter::shutdown_mmio::<", 0x1E, "bk1080_startup_adapter::shutdown_with_io::<", False, False),
            ("bk1080_startup_adapter::shutdown_with_io::<", 0x08, "bk1080_startup_adapter::shutdown_transaction::<", False, False),
            ("bk1080_startup_adapter::shutdown_transaction::<", 0x08, "I2cTransport>::start", False, True),
            ("::new_target", 0x04, "DelayRef>>::new", True, True),
        )
        for caller, offset, callee, suffix, callee_suffix in cases:
            with self.subTest(caller=caller, callee=callee):
                mutated = self.redirect_call(caller, offset, callee, suffix, callee_suffix)
                with self.assertRaises(witness.WitnessError):
                    self.validate_emission(mutated)

    def test_constructor_cannot_call_a_different_real_entry(self):
        mutated = self.redirect_call(
            "::new_target", 0x04, "DelayRef>>::new", True, True,
            replacement_callee="DelayRef>>>::new",
        )
        with self.assertRaises(witness.WitnessError):
            self.validate_emission(mutated)

    def test_fm_frames_and_entry_traps(self):
        cases = (
            ("StartupIo>::bk1080_shutdown", True, 0, r"\bpush\b", "de00", "udf #0"),
            ("StartupIo>::bk1080_shutdown", True, 0x10, r"\badd\s+sp,\s*#8", "b003", "add sp, #12"),
            ("bk1080_startup_adapter::shutdown_mmio::<", False, 0, r"\bpush\b", "e7fe", "b.n {address:x}"),
            ("bk1080_startup_adapter::shutdown_mmio::<", False, 0x38, r"\badd\s+sp,\s*#40", "b009", "add sp, #36"),
            ("bk1080_startup_adapter::shutdown_with_io::<", False, 0x2C, r"\badd\s+sp,\s*#16", "b003", "add sp, #12"),
        )
        for fragment, suffix, offset, expected, encoding, instruction in cases:
            with self.subTest(fragment=fragment, offset=offset):
                address, _ = self.entry(fragment, suffix)
                mutated = self.replace_instruction(
                    fragment, offset, expected, encoding,
                    instruction.format(address=address + offset), suffix,
                )
                with self.assertRaises(witness.WitnessError):
                    self.validate_emission(mutated)

    def test_fm_result_and_gpio_rmw_are_not_bypassed(self):
        cases = (
            ("bk1080_startup_adapter::shutdown_mmio::<", 0x36, r"\bldr\s+r0,\s*\[sp,\s*#4\]", "9800", "ldr r0, [sp, #0]"),
            ("bk1080_startup_adapter::shutdown_with_io::<", 0x20, r"\bmovs\s+r3,\s*#1", "2300", "movs r3, #0"),
            ("bk1080_startup_adapter::shutdown_with_io::<", 0x24, r"\borrs\s+r2,\s*r3", "4312", "orrs r2, r2"),
            ("bk1080_startup_adapter::shutdown_with_io::<", 0x2A, r"\bldr\s+r0,\s*\[sp,\s*#12\]", "9802", "ldr r0, [sp, #8]"),
        )
        for fragment, offset, expected, encoding, instruction in cases:
            with self.subTest(offset=offset):
                mutated = self.replace_instruction(fragment, offset, expected, encoding, instruction)
                with self.assertRaises(witness.WitnessError):
                    self.validate_emission(mutated)

    def test_fm_gpio_address_access_and_keypad_restore(self):
        cases = (
            ("bk1080_startup_adapter::shutdown_with_io::<", False, 0x30, r"\.word\s+0x40060800", "40060804", ".word 0x40060804"),
            ("bk1080_startup_adapter::WordIo>::read_word", True, 4, r"\bldr\s+r0,\s*\[r1,\s*#0\]", "2000", "movs r0, #0"),
            ("bk1080_startup_adapter::WordIo>::write_word", True, 4, r"\bstr\s+r2,\s*\[r1,\s*#0\]", "6008", "str r0, [r1, #0]"),
            ("::handoff_to_keypad", True, 0x10, r"\bstrb\s+r1,\s*\[r0,\s*#8\]", "7241", "strb r1, [r0, #9]"),
            ("::restore_keypad_mode", True, 0x14, r"\blsls\s+r2,\s*r1,\s*#11", "028a", "lsls r2, r1, #10"),
        )
        for fragment, suffix, offset, expected, encoding, instruction in cases:
            with self.subTest(fragment=fragment, offset=offset):
                mutated = self.replace_instruction(
                    fragment, offset, expected, encoding, instruction, suffix,
                )
                with self.assertRaises(witness.WitnessError):
                    self.validate_emission(mutated)

    def test_fm_delay_dispatch_and_linked_vtable(self):
        delay = "bk1080_shutdown::DelayRef as startup_adapter_witness::i2c_bitbang_flow::target_gpio::Delay1us>::delay_1us"
        mutated = self.replace_instruction(
            delay, 0x0A, r"\bldr\s+r1,\s*\[r1,\s*#12\]", "6889", "ldr r1, [r1, #8]", True,
        )
        with self.assertRaises(witness.WitnessError):
            self.validate_emission(mutated)

        main, _ = self.entry("witness_main")
        literal = re.search(
            rf"(?m)^{main + 0x48:x}:\s+[0-9a-f]+\s+\.word\s+0x([0-9a-f]+)\s*$",
            self.disassembly,
        )
        self.assertIsNotNone(literal)
        table = int(literal.group(1), 16)
        target, _ = self.entry(
            "LinkOnlyDelay as startup_adapter_witness::i2c_bitbang_flow::target_gpio::Delay1us>::delay_1us",
            True,
        )
        original_method = (target | 1).to_bytes(4, "little").hex()
        contents = self.run_tool("objdump", "-s", "-j", ".text", str(ELF))
        line = re.compile(
            rf"(?m)^(\s*{table:x}\s+00000000\s+00000000\s+01000000\s+)({original_method})(\s+.*)$"
        )
        self.assertEqual(len(line.findall(contents)), 1)
        mutated_contents = line.sub(
            lambda match: match.group(1) + target.to_bytes(4, "little").hex() + match.group(3),
            contents, count=1,
        )
        with self.assertRaises(witness.WitnessError):
            self.validate_emission(self.disassembly, mutated_contents)

    def test_fm_handoff_and_dispatch_bypasses(self):
        mmio, _ = self.entry("bk1080_startup_adapter::shutdown_mmio::<")
        handoff = mmio + 0x32
        # A branch to the result reload skips keypad handoff while leaving it in the listing.
        mutated = self.bypass_call(
            handoff, r"\badd\s+r0,\s*sp,\s*#28",
            r"\bbl\s+[0-9a-f]+\s+<.*handoff_to_keypad>",
        )
        with self.assertRaises(witness.WitnessError):
            self.validate_emission(mutated)

    def test_fm_transaction_control_flow_and_bytes(self):
        fragment = "bk1080_startup_adapter::shutdown_transaction::<"
        cases = (
            (0x00, r"\bpush\b", "de00", "udf #0"),
            (0x34, r"\bmovs\s+r1,\s*#128", "2181", "movs r1, #129"),
            (0x38, r"\bmovs\s+r1,\s*#4", "2105", "movs r1, #5"),
            (0x3C, r"\bmovs\s+r1,\s*#2", "2103", "movs r1, #3"),
            (0x40, r"\bmovs\s+r1,\s*#65", "2140", "movs r1, #64"),
            (0x20, r"\bbeq\.n\b", "e7fe", "b.n {address:x}"),
            (0x20, r"\bbeq\.n\b", "d0ff", "beq.n {next_address:x}"),
            (0x88, r"\bbeq\.n\b", "e7fe", "b.n {address:x}"),
            (0xDE, r"\bbeq\.n\b", "e7fe", "b.n {address:x}"),
            (0xB6, r"\badd\s+sp,\s*#64", "b00f", "add sp, #60"),
        )
        entry, _ = self.entry(fragment)
        for offset, expected, encoding, instruction in cases:
            with self.subTest(offset=offset):
                mutated = self.replace_instruction(
                    fragment, offset, expected, encoding,
                    instruction.format(address=entry + offset, next_address=entry + offset + 2),
                )
                with self.assertRaises(witness.WitnessError):
                    self.validate_emission(mutated)

        for offset, target, previous in (
            (0x08, "I2cTransport>::start", r"\bstr\s+r0,\s*\[sp,\s*#8\]"),
            (0x92, "I2cTransport>::write_byte", r"\bldrb\s+r1,\s*\[r1,\s*#1\]"),
            (0xA4, "I2cTransport>::stop", r"\bldr\s+r0,\s*\[sp,\s*#8\]"),
            (0xE4, "I2cTransport>::stop", r"\bldr\s+r0,\s*\[sp,\s*#8\]"),
        ):
            with self.subTest(offset=offset, target=target):
                address = entry + offset
                mutated = self.bypass_call(
                    address, previous,
                    rf"\bbl\s+[0-9a-f]+\s+<.*{re.escape(target)}>",
                )
                with self.assertRaises(witness.WitnessError):
                    self.validate_emission(mutated)

    def test_fm_backend_checks_actual_result(self):
        backend, name = self.entry("board_sequence::Backend>::call")
        body = witness.function_entry_body(self.disassembly, backend, name)
        calls = [
            ins for ins in witness.parse_instructions(body)
            if ins[2] == "bl" and "StartupIo>::bk1080_shutdown" in ins[4]
        ]
        self.assertEqual(len(calls), 1)
        address = calls[0][0] + 4
        mutated = self.replace_at(
            self.disassembly, address, r"\bmov\s+r1,\s*r0",
            f"{address:x}:\t2100\tmovs r1, #0",
        )
        with self.assertRaises(witness.WitnessError):
            self.validate_emission(mutated)

        call = calls[0][0]
        mutated = self.bypass_call(
            call, r"\bldr\s+r2,\s*\[r2,\s*#24\]",
            r"\bbl\s+[0-9a-f]+\s+<.*StartupIo>::bk1080_shutdown>",
        )
        with self.assertRaises(witness.WitnessError):
            self.validate_emission(mutated)


if __name__ == "__main__":
    unittest.main()
