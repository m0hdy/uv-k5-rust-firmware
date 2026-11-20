import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from check_firmware_entry import EntryError, inspect_disassembly  # noqa: E402


DISASSEMBLY = """
000000c0 <Reset_Handler>:
 c0: bl 200 <firmware_main>
 c4: b c4 <Reset_Handler+0x4>
00000200 <firmware_main>:
 200: bl 300 <firmware_integration_entry>
00000300 <firmware_integration_entry>:
 300: bl 400 <firmware_fail_closed>
 304: bl 500 <board::StartupIo>::systick>
 308: bl 510 <board::StartupIo>::portcon>
 30c: bl 520 <board::StartupIo>::gpio>
 310: bl 530 <board::StartupIo>::backlight_init_hardware>
 314: bl 540 <board::StartupIo>::adc_init>
 318: bl 550 <board::StartupIo>::st7565_init>
 31c: bl 600 <eeprom_read_transport::execute_read::<TargetBootIo>>
 320: bl 600 <eeprom_read_transport::execute_read::<TargetBootIo>>
 324: bl 600 <eeprom_read_transport::execute_read::<TargetBootIo>>
 328: bl 700 <adc_samples::decode_adc_sample>
 32c: bl 710 <adc_samples::calibrated_battery_voltage>
 330: bl 720 <startup_version_display::startup_version_display_plan>
 334: bl 730 <startup_version_row_plan::compose_startup_version_row>
00000400 <firmware_fail_closed>:
 400: cpsid i
 402: wfi
 404: b 402 <firmware_fail_closed+0x2>
00000500 <board::StartupIo>::systick>:
 500: bx lr
00000510 <board::StartupIo>::portcon>:
 510: bx lr
00000520 <board::StartupIo>::gpio>:
 520: bx lr
00000530 <board::StartupIo>::backlight_init_hardware>:
 530: bx lr
00000540 <board::StartupIo>::adc_init>:
 540: bx lr
00000550 <board::StartupIo>::st7565_init>:
 550: bx lr
00000600 <eeprom_read_transport::execute_read::<TargetBootIo>>:
 600: bx lr
00000700 <adc_samples::decode_adc_sample>:
 700: bx lr
00000710 <adc_samples::calibrated_battery_voltage>:
 710: bx lr
00000720 <startup_version_display::startup_version_display_plan>:
 720: bx lr
00000730 <startup_version_row_plan::compose_startup_version_row>:
 730: bx lr
00000800 <SysTick_Handler>:
 800: bx lr
 804: .word 0xe000e180
"""

SYMBOLS = """
GpioI2cIo<TargetRegisters, systick_delay_adapter::SysTickDelay>
systick_delay_adapter::SysTickDelay<VolatileSysTickCounter>
"""


class FirmwareEntryTests(unittest.TestCase):
    def test_selected_entry_reaches_all_required_stages(self):
        inspect_disassembly(DISASSEMBLY, SYMBOLS)

    def test_disconnected_firmware_main_is_rejected(self):
        broken = DISASSEMBLY.replace(
            "200: bl 300 <firmware_integration_entry>",
            "200: b 200 <firmware_main>",
        )
        with self.assertRaisesRegex(EntryError, "does not call firmware_integration_entry"):
            inspect_disassembly(broken, SYMBOLS)

    def test_unmasked_adc_nvic_is_rejected(self):
        broken = DISASSEMBLY.replace(".word 0xe000e180", ".word 0xe000e100")
        with self.assertRaisesRegex(EntryError, "SARADC must remain masked"):
            inspect_disassembly(broken, SYMBOLS)


if __name__ == "__main__":
    unittest.main()
