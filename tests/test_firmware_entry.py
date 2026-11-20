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
 518: bl 700 <eeprom_read_transport::execute_read::<TargetBootIo>>
 51a: ldr r0, [sp, #0]
 51c: cmp r0, #2
 51e: beq.n 524 <coordinate_with_welcome::<TargetBootIo>+0x24>
 520: b.n 580 <coordinate_with_welcome::<TargetBootIo>+0x80>
 524: bl 700 <eeprom_read_transport::execute_read::<TargetBootIo>>
 526: ldr r0, [sp, #0]
 528: cmp r0, #2
 52a: bne.n 580 <coordinate_with_welcome::<TargetBootIo>+0x80>
 52c: bl 700 <eeprom_read_transport::execute_read::<TargetBootIo>>
 52e: ldr r0, [sp, #0]
 530: cmp r0, #2
 532: bne.n 580 <coordinate_with_welcome::<TargetBootIo>+0x80>
 534: bl 710 <adc_samples::decode_adc_sample>
 538: bl 720 <adc_samples::calibrated_battery_voltage>
 53c: bl 730 <startup_version_display::startup_version_display_plan>
 540: bl 740 <startup_version_row_plan::compose_startup_version_row>
 544: bx lr
 580: bx lr
00000600 <board::StartupIo>::systick>:
 600: str r0, [r1]
 602: bx lr
00000610 <board::StartupIo>::portcon>:
 610: bl 750 <portcon_plan>
 614: bx lr
00000620 <board::StartupIo>::gpio>:
 620: str r0, [r1]
 622: bx lr
00000630 <board::StartupIo>::backlight_init_hardware>:
 630: str r0, [r1]
 632: bx lr
00000640 <board::StartupIo>::adc_init>:
 640: str r0, [r1]
 642: bx lr
 644: .word 0xe000e180
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
 750: str r0, [r1]
 752: bx lr
00000760 <display_startup>:
 760: str r0, [r1]
 762: bl 770 <SysTickDelay::delay_ms>
 766: bx lr
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
            "620: str r0, [r1]", "620: bx lr", 1
        )
        with self.assertRaisesRegex(EntryError, "gpio executor has no reachable hardware effect"):
            self.inspect(broken)

    def test_unchecked_eeprom_result_is_rejected(self):
        broken = DISASSEMBLY.replace(
            "51e: beq.n 524", "51e: nop", 1
        )
        with self.assertRaisesRegex(
            EntryError, "not conditionally checked|not checked|exactly 3 times"
        ):
            self.inspect(broken)

    def test_unmasked_adc_nvic_is_rejected(self):
        broken = DISASSEMBLY.replace("0xe000e180", "0xe000e100")
        with self.assertRaisesRegex(EntryError, "SARADC must remain masked"):
            self.inspect(broken)


if __name__ == "__main__":
    unittest.main()
