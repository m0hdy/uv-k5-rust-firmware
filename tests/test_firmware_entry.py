import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from check_firmware_entry import EntryError, inspect_disassembly  # noqa: E402


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


if __name__ == "__main__":
    unittest.main()
