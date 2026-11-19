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

CONNECTED_DISPATCH = """
00000100 <Reset_Handler>:
 100: f000 f800 bl 200 <witness_main>
00000200 <witness_main>:
 200: f000 f800 bl 300 <run_hardware>
00000300 <run_hardware>:
 300: f000 f800 bl 400 <board_startup_adapters::run::<Hardware>>
00000400 <board_startup_adapters::run::<Hardware>>:
 400: f000 f800 bl 500 <board_sequence::run::<Backend>>
00000500 <board_sequence::run::<Backend>>:
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
00000600 <startup::SequencerBackend<Hardware> as startup::board_sequence::Backend>::call>:
 600: 0081 lsls r1, r0, #2
 602: a001 add r0, pc, #4 @ (adr r0, 610 <SequencerBackend as Backend>::call+0x10)
 604: 5840 ldr r0, [r0, r1]
 606: 4687 mov pc, r0
 610: 1021 .word 0x1021
 614: 1031 .word 0x1031
 618: 1041 .word 0x1041
 61c: 1051 .word 0x1051
 1020: f000 f800 bl 700 <HardwareIo as StartupIo>::device_clock_gates>
 1030: f000 f800 bl 710 <HardwareIo as StartupIo>::systick>
 1040: f000 f800 bl 720 <HardwareIo as StartupIo>::portcon>
 1050: f000 f800 bl 730 <HardwareIo as StartupIo>::gpio>
00000700 <<HardwareIo as StartupIo>::device_clock_gates>:
 700: f000 f800 bl 800 <apply_device_clock_gates>
00000710 <<HardwareIo as StartupIo>::systick>:
 710: f000 f800 bl 810 <apply_to_hardware>
00000720 <<HardwareIo as StartupIo>::portcon>:
 720: f000 f800 bl 820 <apply_portcon_startup_plan>
00000730 <<HardwareIo as StartupIo>::gpio>:
 730: f000 f800 bl 830 <apply_gpio_startup_plan>
"""

DISCONNECTED_DISPATCH = CONNECTED_DISPATCH.replace(
    """ 600: 0081 lsls r1, r0, #2
 602: a001 add r0, pc, #4 @ (adr r0, 610 <SequencerBackend as Backend>::call+0x10)
 604: 5840 ldr r0, [r0, r1]
 606: 4687 mov pc, r0
 610: 1021 .word 0x1021
 614: 1031 .word 0x1031
 618: 1041 .word 0x1041
 61c: 1051 .word 0x1051""",
    " 600: 4770 bx lr",
)

SWAPPED_DISPATCH = CONNECTED_DISPATCH.replace(
    " 610: 1021 .word 0x1021\n 614: 1031 .word 0x1031",
    " 610: 1031 .word 0x1031\n 614: 1021 .word 0x1021",
)


class StartupWitnessCheckerTests(unittest.TestCase):
    def test_rejects_disconnected_entry_when_downstream_chain_is_retained(self):
        self.assertIn("run_hardware", DISCONNECTED_ENTRY)
        self.assertIn("board_startup_adapters::run", DISCONNECTED_ENTRY)
        self.assertIn("board_sequence::run", DISCONNECTED_ENTRY)

        with self.assertRaisesRegex(WitnessError, "witness_main.*run_hardware"):
            validate_call_path(DISCONNECTED_ENTRY)

    def test_rejects_disconnected_backend_dispatcher_with_methods_retained(self):
        self.assertIn("StartupIo>::device_clock_gates", DISCONNECTED_DISPATCH)
        self.assertIn("StartupIo>::gpio", DISCONNECTED_DISPATCH)

        with self.assertRaisesRegex(WitnessError, "sequencer backend.*dispatch"):
            validate_call_path(DISCONNECTED_DISPATCH)

    def test_rejects_swapped_backend_dispatch_targets(self):
        with self.assertRaisesRegex(WitnessError, "variant 0.*device_clock_gates"):
            validate_call_path(SWAPPED_DISPATCH)

    def test_accepts_linked_backend_dispatch_to_matching_hardware_methods(self):
        validate_call_path(CONNECTED_DISPATCH)


if __name__ == "__main__":
    unittest.main()
