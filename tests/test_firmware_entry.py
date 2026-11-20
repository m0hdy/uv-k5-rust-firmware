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
 304: bl 800 <coordinate_with_welcome::<TargetBootIo>>
00000400 <firmware_fail_closed>:
 400: cpsid i
 402: wfi
 404: b 402 <firmware_fail_closed+0x2>
00000500 <eeprom_read_transport::execute_read::<TargetBootIo>>:
 500: bx lr
00000600 <adc_samples::decode_adc_sample>:
 600: bx lr
00000610 <adc_samples::calibrated_battery_voltage>:
 610: bx lr
00000620 <startup_version_display::startup_version_display_plan>:
 620: bx lr
00000630 <startup_version_row_plan::compose_startup_version_row>:
 630: bx lr
00000700 <board::StartupIo>::systick>:
 700: bx lr
00000710 <board::StartupIo>::portcon>:
 710: bx lr
00000720 <board::StartupIo>::gpio>:
 720: bx lr
00000730 <board::StartupIo>::backlight_init_hardware>:
 730: bx lr
00000740 <board::StartupIo>::adc_init>:
 740: bx lr
00000750 <board::StartupIo>::st7565_init>:
 750: bx lr
00000800 <coordinate_with_welcome::<TargetBootIo>>:
 800: bl 700 <board::StartupIo>::systick>
 804: bl 710 <board::StartupIo>::portcon>
 808: bl 720 <board::StartupIo>::gpio>
 80c: bl 730 <board::StartupIo>::backlight_init_hardware>
 810: bl 740 <board::StartupIo>::adc_init>
 814: bl 750 <board::StartupIo>::st7565_init>
 818: bl 500 <eeprom_read_transport::execute_read::<TargetBootIo>>
 81c: bl 500 <eeprom_read_transport::execute_read::<TargetBootIo>>
 820: bl 500 <eeprom_read_transport::execute_read::<TargetBootIo>>
 824: bl 600 <adc_samples::decode_adc_sample>
 828: bl 610 <adc_samples::calibrated_battery_voltage>
 82c: bl 620 <startup_version_display::startup_version_display_plan>
 830: bl 630 <startup_version_row_plan::compose_startup_version_row>
00000900 <SysTick_Handler>:
 900: bx lr
 904: .word 0xe000e180
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
