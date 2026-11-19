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
 506: 2101 movs r1, #1
 508: f000 f800 bl 600 <<Backend>::call>
 50c: 2102 movs r1, #2
 50e: f000 f800 bl 600 <<Backend>::call>
 512: 2103 movs r1, #3
 514: f000 f800 bl 600 <<Backend>::call>
 518: 2104 movs r1, #4
 51a: f000 f800 bl 600 <<Backend>::call>
 51e: 2105 movs r1, #5
 520: f000 f800 bl 600 <<Backend>::call>
 524: 2106 movs r1, #6
 526: f000 f800 bl 600 <<Backend>::call>
000005f6 <startup::SequencerBackend<Hardware> as startup::board_sequence::Backend>::call>:
 5f6: b580 push {r7, lr}
 5f8: af00 add r7, sp, #0
 5fa: b084 sub sp, #16
 5fc: 9001 str r0, [sp, #4]
 5fe: 9102 str r1, [sp, #8]
 600: 7980 ldrb r0, [r0, #6]
 602: 07c0 lsls r0, r0, #31
 604: 2800 cmp r0, #0
 606: d112 bne.n 670 <SequencerBackend as Backend>::call+0x70
 608: e7ff b.n 60a <SequencerBackend as Backend>::call+0xa
 60a: 9802 ldr r0, [sp, #8]
 60c: b2c0 uxtb r0, r0
 60e: 9000 str r0, [sp, #0]
 610: 2803 cmp r0, #3
 612: d81d bhi.n 650 <SequencerBackend as Backend>::call+0x50
 614: 9800 ldr r0, [sp, #0]
 616: 0081 lsls r1, r0, #2
 618: a001 add r0, pc, #4 @ (adr r0, 620 <SequencerBackend as Backend>::call+0x20)
 61a: 5840 ldr r0, [r0, r1]
 61c: 4687 mov pc, r0
 620: 1021 .word 0x1021
 624: 1031 .word 0x1031
 628: 1041 .word 0x1041
 62c: 1051 .word 0x1051
 650: 9902 ldr r1, [sp, #8]
 652: 9801 ldr r0, [sp, #4]
 654: 6800 ldr r0, [r0, #0]
 656: f000 f8f3 bl 800 <Hardware as StartupIo>::placeholder>
 65a: e709 b.n 670 <SequencerBackend as Backend>::call+0x70
 670: b004 add sp, #16
 672: bd80 pop {r7, pc}
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

ENTRY_SELF_LOOP = CONNECTED_DISPATCH.replace(
    " 200: f000 f800 bl 300 <run_hardware>",
    " 200: e7fe b.n 200 <witness_main>\n"
    " 202: f000 f800 bl 300 <run_hardware>",
)

WRAPPER_EARLY_RETURN = CONNECTED_DISPATCH.replace(
    " 300: f000 f800 bl 400 <board_startup_adapters::run::<Hardware>>",
    " 300: 4770 bx lr\n"
    " 302: f000 f800 bl 400 <board_startup_adapters::run::<Hardware>>",
)

HARDWARE_WRAPPER_EARLY_RETURN = CONNECTED_DISPATCH.replace(
    " 700: f000 f800 bl 800 <apply_device_clock_gates>",
    " 700: 4770 bx lr\n"
    " 702: f000 f800 bl 800 <apply_device_clock_gates>",
)

SKIPPED_SEQUENCER_DISPATCH = CONNECTED_DISPATCH.replace(
    " 500: 2100 movs r1, #0",
    " 500: e000 b.n 506 <board_sequence::run+0x6>",
)

DISCONNECTED_DISPATCH = CONNECTED_DISPATCH.replace(
    " 614: 9800 ldr r0, [sp, #0]",
    " 614: 4770 bx lr",
)

DISCONNECTED_BACKEND_PREFIX = CONNECTED_DISPATCH.replace(
    " 600: 7980 ldrb r0, [r0, #6]",
    " 600: 4770 bx lr\n"
    " 602: 7980 ldrb r0, [r0, #6]",
)

CONSTANT_VARIANT_RELOAD = CONNECTED_DISPATCH.replace(
    " 614: 9800 ldr r0, [sp, #0]",
    " 614: 2000 movs r0, #0",
)

OVERWRITTEN_INCOMING_VARIANT = CONNECTED_DISPATCH.replace(
    " 5fe: 9102 str r1, [sp, #8]",
    " 5fe: 9202 str r2, [sp, #8]",
)

ALWAYS_TAKEN_BACKEND_BRANCH = CONNECTED_DISPATCH.replace(
    " 610: 2803 cmp r0, #3\n 612: d81d bhi.n 650",
    " 610: 4280 cmp r0, r0\n 612: d01d beq.n 650",
)

UNSUPPORTED_BACKEND_CONDITIONAL = CONNECTED_DISPATCH.replace(
    " 610: 2803 cmp r0, #3\n 612: d81d bhi.n 650",
    " 610: 4288 cmp r0, r1\n 612: d02b beq.n 650",
)

WRONG_STOPPED_GUARD_INPUT = CONNECTED_DISPATCH.replace(
    " 600: 7980 ldrb r0, [r0, #6]",
    " 600: 7900 ldrb r0, [r0, #4]",
)

WRONG_STOPPED_GUARD_TARGET = CONNECTED_DISPATCH.replace(
    " 606: d112 bne.n 670",
    " 606: d123 bne.n 650",
)

WRONG_VARIANT_GUARD_INPUT = CONNECTED_DISPATCH.replace(
    " 60a: 9802 ldr r0, [sp, #8]",
    " 60a: 9801 ldr r0, [sp, #4]",
)

WRONG_VARIANT_GUARD_TARGET = CONNECTED_DISPATCH.replace(
    " 612: d81d bhi.n 650",
    " 612: d82d bhi.n 670",
)

SWAPPED_DISPATCH = CONNECTED_DISPATCH.replace(
    " 620: 1021 .word 0x1021\n 624: 1031 .word 0x1031",
    " 620: 1031 .word 0x1031\n 624: 1021 .word 0x1021",
)

UNREACHABLE_DISPATCH = CONNECTED_DISPATCH.replace(
    " 1020: f000 f800 bl 700 <HardwareIo as StartupIo>::device_clock_gates>",
    " 1020: 4770 bx lr\n"
    " 1022: f000 f800 bl 700 <HardwareIo as StartupIo>::device_clock_gates>",
)

BRANCHED_DISPATCH = CONNECTED_DISPATCH.replace(
    " 1020: f000 f800 bl 700 <HardwareIo as StartupIo>::device_clock_gates>",
    " 1020: e000 b.n 1026 <unrelated>\n"
    " 1022: f000 f800 bl 700 <HardwareIo as StartupIo>::device_clock_gates>",
)


class StartupWitnessCheckerTests(unittest.TestCase):
    def test_rejects_disconnected_entry_when_downstream_chain_is_retained(self):
        self.assertIn("run_hardware", DISCONNECTED_ENTRY)
        self.assertIn("board_startup_adapters::run", DISCONNECTED_ENTRY)
        self.assertIn("board_sequence::run", DISCONNECTED_ENTRY)

        with self.assertRaisesRegex(WitnessError, "witness_main.*run_hardware"):
            validate_call_path(DISCONNECTED_ENTRY)

    def test_rejects_witness_entry_self_loop_with_downstream_chain_retained(self):
        self.assertIn("board_sequence::run", ENTRY_SELF_LOOP)

        with self.assertRaisesRegex(WitnessError, "witness_main.*run_hardware"):
            validate_call_path(ENTRY_SELF_LOOP)

    def test_rejects_wrapper_call_after_early_return(self):
        with self.assertRaisesRegex(
            WitnessError, "run_hardware.*board_startup_adapters::run"
        ):
            validate_call_path(WRAPPER_EARLY_RETURN)

    def test_rejects_hardware_adapter_call_after_early_return(self):
        with self.assertRaisesRegex(
            WitnessError, "target device_clock_gates.*no reachable linked call"
        ):
            validate_call_path(HARDWARE_WRAPPER_EARLY_RETURN)

    def test_rejects_skipped_sequencer_dispatch(self):
        with self.assertRaisesRegex(WitnessError, "sequencer.*reachable ordered"):
            validate_call_path(SKIPPED_SEQUENCER_DISPATCH)

    def test_rejects_disconnected_backend_dispatcher_with_methods_retained(self):
        self.assertIn("StartupIo>::device_clock_gates", DISCONNECTED_DISPATCH)
        self.assertIn("StartupIo>::gpio", DISCONNECTED_DISPATCH)

        with self.assertRaisesRegex(WitnessError, "sequencer backend.*dispatch"):
            validate_call_path(DISCONNECTED_DISPATCH)

    def test_rejects_dispatch_prefix_disconnected_at_function_entry(self):
        self.assertIn("600: 4770 bx lr", DISCONNECTED_BACKEND_PREFIX)
        self.assertIn("61c: 4687 mov pc, r0", DISCONNECTED_BACKEND_PREFIX)
        self.assertIn("StartupIo>::gpio", DISCONNECTED_BACKEND_PREFIX)

        with self.assertRaisesRegex(WitnessError, "sequencer backend.*dispatch"):
            validate_call_path(DISCONNECTED_BACKEND_PREFIX)

    def test_rejects_dispatch_after_always_taken_conditional_branch(self):
        self.assertIn("beq.n 650", ALWAYS_TAKEN_BACKEND_BRANCH)
        self.assertIn("610: 4280 cmp r0, r0", ALWAYS_TAKEN_BACKEND_BRANCH)
        self.assertIn("StartupIo>::gpio", ALWAYS_TAKEN_BACKEND_BRANCH)

        with self.assertRaisesRegex(WitnessError, "sequencer backend.*dispatch"):
            validate_call_path(ALWAYS_TAKEN_BACKEND_BRANCH)

    def test_rejects_unsupported_forward_conditional_before_dispatch(self):
        self.assertIn("cmp r0, r1", UNSUPPORTED_BACKEND_CONDITIONAL)
        self.assertIn("beq.n 650", UNSUPPORTED_BACKEND_CONDITIONAL)

        with self.assertRaisesRegex(WitnessError, "sequencer backend.*dispatch"):
            validate_call_path(UNSUPPORTED_BACKEND_CONDITIONAL)

    def test_rejects_mismatched_stopped_guard_input(self):
        with self.assertRaisesRegex(WitnessError, "sequencer backend.*dispatch"):
            validate_call_path(WRONG_STOPPED_GUARD_INPUT)

    def test_rejects_misdirected_stopped_guard(self):
        with self.assertRaisesRegex(WitnessError, "sequencer backend.*dispatch"):
            validate_call_path(WRONG_STOPPED_GUARD_TARGET)

    def test_rejects_mismatched_variant_guard_input(self):
        with self.assertRaisesRegex(WitnessError, "sequencer backend.*dispatch"):
            validate_call_path(WRONG_VARIANT_GUARD_INPUT)

    def test_rejects_constant_variant_index_reload(self):
        with self.assertRaisesRegex(WitnessError, "ordered indexed dispatch"):
            validate_call_path(CONSTANT_VARIANT_RELOAD)

    def test_rejects_overwritten_incoming_variant(self):
        with self.assertRaisesRegex(WitnessError, "ordered indexed dispatch"):
            validate_call_path(OVERWRITTEN_INCOMING_VARIANT)

    def test_rejects_misdirected_variant_guard(self):
        with self.assertRaisesRegex(WitnessError, "sequencer backend.*dispatch"):
            validate_call_path(WRONG_VARIANT_GUARD_TARGET)

    def test_rejects_swapped_backend_dispatch_targets(self):
        with self.assertRaisesRegex(WitnessError, "variant 0.*device_clock_gates"):
            validate_call_path(SWAPPED_DISPATCH)

    def test_rejects_backend_call_after_early_return(self):
        with self.assertRaisesRegex(WitnessError, "variant 0.*does not reach"):
            validate_call_path(UNREACHABLE_DISPATCH)

    def test_rejects_backend_call_after_unsupported_branch(self):
        with self.assertRaisesRegex(WitnessError, "variant 0.*does not reach"):
            validate_call_path(BRANCHED_DISPATCH)

    def test_accepts_linked_backend_dispatch_to_matching_hardware_methods(self):
        validate_call_path(CONNECTED_DISPATCH)


if __name__ == "__main__":
    unittest.main()
