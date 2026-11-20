import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from check_firmware_entry import (  # noqa: E402
    EntryError,
    error_store_kinds,
    function_has_mmio_store,
    inspect_disassembly,
)


DISASSEMBLY = """
000000c0 <Reset_Handler>:
 c0: b672 cpsid i
 c2: bl 200 <firmware_main>
 c6: b.n c6 <Reset_Handler+0x6>
00000200 <firmware_main>:
 200: bl 300 <firmware_integration_entry>
00000300 <firmware_integration_entry>:
 300: bl 500 <coordinate_with_welcome::<TargetBootIo>>
 304: bl 400 <firmware_fail_closed>
00000400 <firmware_fail_closed>:
 400: cpsid i
 402: wfi
 404: b.n 402 <firmware_fail_closed+0x2>
00000500 <coordinate_with_welcome::<TargetBootIo>>:
 500: bl 600 <board::StartupIo>::systick>
 504: bl 610 <board::StartupIo>::portcon>
 508: bl 620 <board::StartupIo>::gpio>
 50c: bl 630 <board::StartupIo>::backlight_init_hardware>
 510: bl 640 <board::StartupIo>::adc_init>
 514: bl 650 <board::StartupIo>::st7565_init>
 516: add r0, sp, #0
 518: bl 700 <eeprom_read_transport::execute_read::<TargetBootIo>>
 51c: ldr r0, [sp, #0]
 51e: cmp r0, #2
 520: beq.n 526 <coordinate_with_welcome::<TargetBootIo>+0x26>
 522: b.n 590 <coordinate_with_welcome::<TargetBootIo>+0x90>
 526: add r0, sp, #0
 528: bl 700 <eeprom_read_transport::execute_read::<TargetBootIo>>
 52c: ldr r0, [sp, #0]
 52e: cmp r0, #2
 530: bne.n 590 <coordinate_with_welcome::<TargetBootIo>+0x90>
 532: add r0, sp, #0
 534: bl 700 <eeprom_read_transport::execute_read::<TargetBootIo>>
 538: ldr r0, [sp, #0]
 53a: cmp r0, #2
 53c: bne.n 590 <coordinate_with_welcome::<TargetBootIo>+0x90>
 53e: bl 710 <adc_samples::decode_adc_sample>
 542: bl 720 <adc_samples::calibrated_battery_voltage>
 546: bl 730 <startup_version_display::startup_version_display_plan>
 54a: bl 740 <startup_version_row_plan::compose_startup_version_row>
 54e: bx lr
 590: bx lr
00000600 <board::StartupIo>::systick>:
 600: ldr r1, [pc, #4]
 602: str r0, [r1]
 604: bx lr
 608: .word 0xe000e010
00000610 <board::StartupIo>::portcon>:
 610: bl 750 <portcon_plan>
 614: bx lr
00000620 <board::StartupIo>::gpio>:
 620: ldr r1, [pc, #4]
 622: str r0, [r1]
 624: bx lr
 628: .word 0x40060000
00000630 <board::StartupIo>::backlight_init_hardware>:
 630: ldr r1, [pc, #4]
 632: str r0, [r1]
 634: bx lr
 638: .word 0x400b4008
00000640 <board::StartupIo>::adc_init>:
 640: ldr r1, [pc, #4]
 642: str r0, [r1]
 644: bx lr
 648: .word 0xe000e180
00000650 <board::StartupIo>::st7565_init>:
 650: bl 760 <display_startup>
 654: bx lr
00000700 <eeprom_read_transport::execute_read::<TargetBootIo>>:
 700: str r0, [r1]
 702: bx lr
00000710 <adc_samples::decode_adc_sample>:
 710: ands r0, r1
 712: bx lr
00000720 <adc_samples::calibrated_battery_voltage>:
 720: adds r0, r1
 722: bx lr
00000730 <startup_version_display::startup_version_display_plan>:
 730: cmp r0, #0
 732: bne.n 736 <startup_version_display::startup_version_display_plan+0x6>
 734: movs r0, #1
 736: bx lr
00000740 <startup_version_row_plan::compose_startup_version_row>:
 740: strb r0, [r1]
 742: bx lr
00000750 <portcon_plan>:
 750: ldr r1, [pc, #4]
 752: str r0, [r1]
 754: bx lr
 758: .word 0x400b0000
00000760 <display_startup>:
 760: ldr r1, [pc, #8]
 762: str r0, [r1]
 764: bl 770 <SysTickDelay::delay_ms>
 768: bx lr
 76c: .word 0x400b8014
00000770 <SysTickDelay<Counter>::delay_ms>:
 770: ldr r0, [r1]
 772: subs r0, #1
 774: bne.n 770 <SysTickDelay<Counter>::delay_ms>
 776: bx lr
00000780 <SysTickDelay<Counter>::delay_1us>:
 780: ldr r0, [r1]
 782: subs r0, #1
 784: bne.n 780 <SysTickDelay<Counter>::delay_1us>
 786: bx lr
00000900 <SysTick_Handler>:
 900: bx lr
"""

_VECTOR_WORDS = [0x20003FF0, 0x000000C1] + [0x00000109] * 13 + [0x00000901]
_VECTOR_WORDS.extend([0x00000109] * (48 - len(_VECTOR_WORDS)))
VECTOR_DUMP = "Contents of section .vector_table:\n" + "\n".join(
    f" {offset:04x} "
    + " ".join(word.to_bytes(4, "little").hex() for word in _VECTOR_WORDS[offset : offset + 4])
    for offset in range(0, len(_VECTOR_WORDS), 4)
)
SYMBOLS = """
GpioI2cIo<TargetRegisters, systick_delay_adapter::SysTickDelay>
systick_delay_adapter::SysTickDelay<VolatileSysTickCounter>
"""


class FirmwareEntryTests(unittest.TestCase):
    def inspect(self, disassembly=DISASSEMBLY, vector_dump=VECTOR_DUMP):
        inspect_disassembly(disassembly, SYMBOLS, vector_dump)

    def test_selected_entry_reaches_all_required_stages(self):
        self.inspect()

    def test_display_initialization_requires_canonical_prerequisite_order(self):
        broken = DISASSEMBLY.replace(
            "500: bl 600 <board::StartupIo>::systick>\n"
            " 504: bl 610 <board::StartupIo>::portcon>\n"
            " 508: bl 620 <board::StartupIo>::gpio>\n"
            " 50c: bl 630 <board::StartupIo>::backlight_init_hardware>\n"
            " 510: bl 640 <board::StartupIo>::adc_init>\n"
            " 514: bl 650 <board::StartupIo>::st7565_init>",
            "500: bl 600 <board::StartupIo>::systick>\n"
            " 504: bl 650 <board::StartupIo>::st7565_init>\n"
            " 508: bl 610 <board::StartupIo>::portcon>\n"
            " 50c: bl 620 <board::StartupIo>::gpio>\n"
            " 510: bl 630 <board::StartupIo>::backlight_init_hardware>\n"
            " 514: bl 640 <board::StartupIo>::adc_init>",
            1,
        )
        with self.assertRaisesRegex(EntryError, "canonical order"):
            self.inspect(broken)

    def test_reset_entry_self_loop_is_rejected(self):
        broken = DISASSEMBLY.replace(
            "c0: b672 cpsid i", "c0: e7fe b.n c0 <Reset_Handler>", 1
        )
        with self.assertRaisesRegex(EntryError, "mask interrupts"):
            self.inspect(broken)

    def test_firmware_main_self_loop_is_rejected(self):
        broken = DISASSEMBLY.replace(
            "200: bl 300 <firmware_integration_entry>",
            "200: e7fe b.n 200 <firmware_main>",
        )
        with self.assertRaisesRegex(EntryError, "self-loop|cannot reach"):
            self.inspect(broken)

    def test_reset_and_systick_vectors_are_checked(self):
        broken = VECTOR_DUMP.replace("c1000000", "c3000000", 1)
        with self.assertRaisesRegex(EntryError, "reset vector"):
            self.inspect(vector_dump=broken)

    def test_noop_hardware_executor_is_rejected(self):
        broken = DISASSEMBLY.replace(
            "620: ldr r1, [pc, #4]", "620: bx lr", 1
        )
        with self.assertRaisesRegex(EntryError, "gpio executor has no reachable hardware effect"):
            self.inspect(broken)

    def test_stack_only_hardware_executor_is_rejected(self):
        broken = DISASSEMBLY.replace(
            "620: ldr r1, [pc, #4]\n 622: str r0, [r1]",
            "620: nop\n 622: str r0, [sp, #0]",
            1,
        )
        with self.assertRaisesRegex(EntryError, "gpio executor has no reachable hardware effect"):
            self.inspect(broken)

    def test_flag_setting_instruction_clobbers_mmio_address(self):
        broken = DISASSEMBLY.replace(
            "620: ldr r1, [pc, #4]\n 622: str r0, [r1]\n 624: bx lr",
            "620: ldr r1, [pc, #4]\n"
            " 622: bics r1, r1\n"
            " 624: str r0, [r1]\n"
            " 626: bx lr",
            1,
        )
        with self.assertRaisesRegex(
            EntryError, "gpio executor has no reachable hardware effect"
        ):
            self.inspect(broken)

    def test_writeback_clobbers_mmio_address(self):
        broken = DISASSEMBLY.replace(
            "620: ldr r1, [pc, #4]\n"
            " 622: str r0, [r1]\n"
            " 624: bx lr\n"
            " 628: .word 0x40060000",
            "620: ldr r1, [pc, #8]\n"
            " 622: ldmia r1!, {r0}\n"
            " 624: str r0, [r1]\n"
            " 626: bx lr\n"
            " 62c: .word 0x5ffffffc",
            1,
        )
        with self.assertRaisesRegex(
            EntryError, "gpio executor has no reachable hardware effect"
        ):
            self.inspect(broken)

    def test_multi_register_load_clobbers_loaded_addresses(self):
        mmio_body = """
00000000 <f>:
 0: ldr r1, [pc, #4]
 2: ldmia r0!, {r1}
 4: str r0, [r1]
 6: bx lr
 8: .word 0x40060000
"""
        error_body = """
00000000 <f>:
 0: add r1, sp, #0
 2: ldmia r0!, {r1}
 4: str r0, [r1]
 6: bx lr
"""
        self.assertFalse(function_has_mmio_store(mmio_body))
        self.assertEqual(error_store_kinds(error_body)[4], "unknown")

    def test_unchecked_eeprom_result_is_rejected(self):
        broken = DISASSEMBLY.replace(
            "520: beq.n 526", "520: nop", 1
        )
        with self.assertRaisesRegex(
            EntryError, "not conditionally checked|not checked|exactly 3 times"
        ):
            self.inspect(broken)

    def test_unmasked_adc_nvic_is_rejected(self):
        broken = DISASSEMBLY.replace("0xe000e180", "0xe000e100")
        with self.assertRaisesRegex(EntryError, "SARADC must remain masked"):
            self.inspect(broken)

    def test_eeprom_error_cannot_jump_to_welcome(self):
        broken = DISASSEMBLY.replace(
            "522: b.n 590", "522: b.n 546", 1
        )
        with self.assertRaisesRegex(EntryError, "first-error path reaches a later effect"):
            self.inspect(broken)

    def test_eeprom_error_cannot_call_welcome_before_return(self):
        broken = DISASSEMBLY.replace(
            "590: bx lr",
            "590: bl 730 <startup_version_display::startup_version_display_plan>\n"
            " 594: bx lr",
            1,
        )
        with self.assertRaisesRegex(EntryError, "first-error path reaches a later effect"):
            self.inspect(broken)

    def test_eeprom_error_rejects_memcpy_to_unknown_destination(self):
        replacements = (
            "590: ldr r0, [pc, #4]\n"
            " 592: movs r2, #4\n"
            " 594: bl 730 <__aeabi_memcpy>\n"
            " 598: bx lr\n"
            " 59a: .word 0x40060000",
            "590: ldr r0, [pc, #8]\n"
            " 592: movs r2, #4\n"
            " 594: bl 730 <__aeabi_memcpy>\n"
            " 598: bx lr\n"
            " 59c: .word 0x40060000",
            "590: add r0, sp, #0\n"
            " 592: movs r2, #65\n"
            " 594: bl 730 <__aeabi_memcpy>\n"
            " 598: bx lr",
        )
        for replacement in replacements:
            with self.subTest(replacement=replacement):
                broken = DISASSEMBLY.replace("590: bx lr", replacement, 1)
                with self.assertRaisesRegex(
                    EntryError, "first-error path reaches an unapproved copy effect"
                ):
                    self.inspect(broken)

    def test_eeprom_error_memcpy_invalidates_overwritten_pointer_slot(self):
        broken = DISASSEMBLY.replace(
            "500: bl 600 <board::StartupIo>::systick>",
            "500: str r0, [sp, #12]\n"
            " 502: bl 600 <board::StartupIo>::systick>",
            1,
        ).replace(
            "590: bx lr",
            "590: add r0, sp, #12\n"
            " 592: ldr r1, [pc, #16]\n"
            " 594: movs r2, #4\n"
            " 596: bl 730 <__aeabi_memcpy>\n"
            " 59a: ldr r1, [sp, #12]\n"
            " 59c: str r0, [r1]\n"
            " 59e: bx lr\n"
            " 5a4: .word 0x40060000",
            1,
        )
        with self.assertRaisesRegex(
            EntryError, "first-error path reaches an unapproved store effect"
        ):
            self.inspect(broken)

    def test_eeprom_error_cannot_write_inline_mmio(self):
        broken = DISASSEMBLY.replace(
            "590: bx lr",
            "590: ldr r1, [pc, #4]\n"
            " 592: str r0, [r1]\n"
            " 594: bx lr\n"
            " 598: .word 0x40060000",
            1,
        )
        with self.assertRaisesRegex(
            EntryError, "first-error path reaches an unapproved store effect"
        ):
            self.inspect(broken)

    def test_eeprom_error_cannot_write_inline_mmio_with_stmia(self):
        broken = DISASSEMBLY.replace(
            "590: bx lr",
            "590: ldr r1, [pc, #4]\n"
            " 592: stmia r1!, {r0}\n"
            " 594: bx lr\n"
            " 598: .word 0x40060000",
            1,
        )
        with self.assertRaisesRegex(
            EntryError, "first-error path reaches an unapproved store effect"
        ):
            self.inspect(broken)

    def test_eeprom_error_rejects_indirect_escape(self):
        broken = DISASSEMBLY.replace("590: bx lr", "590: bx r0", 1)
        with self.assertRaisesRegex(EntryError, "unsupported return or escape"):
            self.inspect(broken)

    def test_eeprom_error_rejects_clobbered_stack_address(self):
        broken = DISASSEMBLY.replace(
            "590: bx lr",
            "590: add r1, sp, #0\n"
            " 592: bics r1, r1\n"
            " 594: str r0, [r1]\n"
            " 596: bx lr",
            1,
        )
        with self.assertRaisesRegex(
            EntryError, "first-error path reaches an unapproved store effect"
        ):
            self.inspect(broken)

    def test_eeprom_error_rejects_store_after_stack_pointer_clobber(self):
        broken = DISASSEMBLY.replace(
            "590: bx lr",
            "590: ldr r1, [pc, #4]\n"
            " 592: mov sp, r1\n"
            " 594: str r0, [sp, #0]\n"
            " 596: bx lr\n"
            " 598: .word 0x40060000",
            1,
        )
        with self.assertRaisesRegex(
            EntryError, "first-error path reaches an unapproved store effect"
        ):
            self.inspect(broken)

    def test_fail_closed_terminal_cannot_reenable_interrupts(self):
        broken = DISASSEMBLY.replace(
            "400: cpsid i\n 402: wfi",
            "400: cpsid i\n 401: cpsie i\n 402: wfi",
            1,
        )
        with self.assertRaisesRegex(EntryError, "must not unmask interrupts"):
            self.inspect(broken)

    def test_fail_closed_terminal_must_be_reachable(self):
        broken = DISASSEMBLY.replace(
            "400: cpsid i\n 402: wfi\n 404: b.n 402",
            "400: bx lr\n 402: cpsid i\n 404: wfi\n 406: b.n 404",
            1,
        )
        with self.assertRaisesRegex(EntryError, "reachable return or escape"):
            self.inspect(broken)

    def test_partial_stack_write_invalidates_only_overlapping_pointer_slot(self):
        overlap = """
00000500 <coordinate_with_welcome::<TargetBootIo>>:
 500: str r0, [sp, #0]
 502: movs r1, #64
 504: add r2, sp, #3
 506: strb r1, [r2]
 508: ldr r0, [sp, #0]
 50a: str r1, [r0]
 50c: bx lr
"""
        untouched = overlap.replace(
            "504: add r2, sp, #3", "504: add r2, sp, #8"
        )
        self.assertEqual(error_store_kinds(overlap)[0x50A], "unknown")
        self.assertEqual(error_store_kinds(untouched)[0x50A], "data")

    def test_address_analysis_widens_looping_values(self):
        mmio_loop = """
00000000 <f>:
 0: ldr r1, [pc, #8]
 2: adds r1, r1, #4
 4: cmp r0, #0
 6: bne.n 2
 8: str r0, [r1]
 a: bx lr
 c: .word 0x20000000
"""
        error_loop = """
00000000 <f>:
 0: adds r1, r1, #4
 2: cmp r0, #0
 4: bne.n 0
 6: bx lr
"""
        self.assertFalse(function_has_mmio_store(mmio_loop))
        self.assertEqual(error_store_kinds(error_loop), {})


if __name__ == "__main__":
    unittest.main()
