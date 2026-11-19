import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from check_startup_witness import WitnessError, validate_call_path  # noqa: E402


DISCONNECTED_ENTRY = """
00000100 <Reset_Handler>:
 100: f000 f800 bl 200 <witness_main>
00000200 <witness_main>:
 200: e7fe b.n 200 <witness_main>
00000300 <startup::run_hardware>:
 300: f000 f800 bl 400 <startup::board_startup_adapters::run::<Hardware>>
00000400 <startup::board_startup_adapters::run::<Hardware>>:
 400: f000 f800 bl 500 <startup::board_sequence::run::<Backend>>
00000500 <startup::board_sequence::run::<Backend>>:
 500: 2100 movs r1, #0
 502: f000 f800 bl 600 <<Backend>::call>
 504: 2101 movs r1, #1
 506: f000 f800 bl 600 <<Backend>::call>
 508: 2102 movs r1, #2
 50a: f000 f800 bl 600 <<Backend>::call>
 50c: 2103 movs r1, #3
 50e: f000 f800 bl 600 <<Backend>::call>
 510: 2104 movs r1, #4
 512: f000 f800 bl 600 <<Backend>::call>
 514: 2105 movs r1, #5
 516: f000 f800 bl 600 <<Backend>::call>
 518: 2106 movs r1, #6
 51a: f000 f800 bl 600 <<Backend>::call>
00000600 <<HardwareIo as StartupIo>::device_clock_gates>:
 600: f000 f800 bl 700 <apply_device_clock_gates>
00000610 <<HardwareIo as StartupIo>::systick>:
 610: f000 f800 bl 710 <apply_to_hardware>
00000620 <<HardwareIo as StartupIo>::portcon>:
 620: f000 f800 bl 720 <apply_portcon_startup_plan>
00000630 <<HardwareIo as StartupIo>::gpio>:
 630: f000 f800 bl 730 <apply_gpio_startup_plan>
"""


class StartupWitnessCheckerTests(unittest.TestCase):
    def test_rejects_disconnected_entry_when_downstream_chain_is_retained(self):
        self.assertIn("run_hardware", DISCONNECTED_ENTRY)
        self.assertIn("board_startup_adapters::run", DISCONNECTED_ENTRY)
        self.assertIn("board_sequence::run", DISCONNECTED_ENTRY)

        with self.assertRaisesRegex(WitnessError, "witness_main.*run_hardware"):
            validate_call_path(DISCONNECTED_ENTRY)


if __name__ == "__main__":
    unittest.main()
