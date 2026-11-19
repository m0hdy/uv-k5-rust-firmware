import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from check_startup_witness import (  # noqa: E402
    WitnessError,
    validate_call_path,
    validate_systick_handler,
)


SYSTICK_RETURN = """
00000100 <SysTick_Handler>:
 100: 4770 bx lr
"""

SYSTICK_SELF_LOOP = """
00000100 <SysTick_Handler>:
 100: e7fe b.n 100 <SysTick_Handler>
 102: 4770 bx lr
"""

SYSTICK_OTHER_BEFORE_RETURN = """
00000100 <SysTick_Handler>:
 100: 2000 movs r0, #0
 102: 4770 bx lr
"""


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
 506: 9800 ldr r0, [sp, #0]
 508: 2101 movs r1, #1
 50a: f000 f800 bl 600 <<Backend>::call>
 50e: 9800 ldr r0, [sp, #0]
 510: 2102 movs r1, #2
 512: f000 f800 bl 600 <<Backend>::call>
 516: 9800 ldr r0, [sp, #0]
 518: 2103 movs r1, #3
 51a: f000 f800 bl 600 <<Backend>::call>
 51e: 9800 ldr r0, [sp, #0]
 520: 2104 movs r1, #4
 522: f000 f800 bl 600 <<Backend>::call>
 526: 9800 ldr r0, [sp, #0]
 528: 2105 movs r1, #5
 52a: f000 f800 bl 600 <<Backend>::call>
 52e: 9800 ldr r0, [sp, #0]
 530: 2106 movs r1, #6
 532: f000 f800 bl 600 <<Backend>::call>
 536: 9801 ldr r0, [sp, #4]
 538: 7800 ldrb r0, [r0, #0]
 53a: 07c0 lsls r0, r0, #31
 53c: 2800 cmp r0, #0
 53e: d807 bne.n 550 <board_sequence::run+0x50>
 540: e00e b.n 560 <board_sequence::run+0x60>
 550: 9800 ldr r0, [sp, #0]
 552: 2107 movs r1, #7
 554: f000 f800 bl 600 <<Backend>::call>
 558: e002 b.n 560 <board_sequence::run+0x60>
 560: 9800 ldr r0, [sp, #0]
 562: e005 b.n 570 <board_sequence::run+0x70>
 570: b004 add sp, #16
 572: bd80 pop {r7, pc}
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
 628: 1071 .word 0x1071
 62c: 1091 .word 0x1091
 650: 9902 ldr r1, [sp, #8]
 652: 9801 ldr r0, [sp, #4]
 654: 6800 ldr r0, [r0, #0]
 656: f000 f8f3 bl 800 <Hardware as StartupIo>::placeholder>
 65a: e709 b.n 670 <SequencerBackend as Backend>::call+0x70
 670: b004 add sp, #16
 672: bd80 pop {r7, pc}
 1020: 9801 ldr r0, [sp, #4]
 1022: 6800 ldr r0, [r0, #0]
 1024: f000 f800 bl 700 <HardwareIo as StartupIo>::device_clock_gates>
 1028: e722 b.n 670 <SequencerBackend as Backend>::call+0x70>
 1030: 9801 ldr r0, [sp, #4]
 1032: 6800 ldr r0, [r0, #0]
 1034: 490d ldr r1, [pc, #52]
 1036: f000 f800 bl 710 <HardwareIo as StartupIo>::systick>
 103a: 4601 mov r1, r0
 103c: a803 add r0, sp, #12
 103e: 7001 strb r1, [r0, #0]
 1040: f000 f800 bl 800 <Result<(), startup_adapter_witness::board_startup_adapters::StartupError>>::is_err>
 1044: 2800 cmp r0, #0
 1046: d10b bne.n 1060 <SequencerBackend as Backend>::call+0x80>
 1048: e712 b.n 670 <SequencerBackend as Backend>::call+0x70>
 1060: 9901 ldr r1, [sp, #4]
 1062: 2001 movs r0, #1
 1064: 7188 strb r0, [r1, #6]
 1066: e703 b.n 670 <SequencerBackend as Backend>::call+0x70>
 1070: 9901 ldr r1, [sp, #4]
 1072: 6808 ldr r0, [r1, #0]
 1074: 7909 ldrb r1, [r1, #4]
 1076: f000 f800 bl 720 <HardwareIo as StartupIo>::portcon>
 107a: e6f9 b.n 670 <SequencerBackend as Backend>::call+0x70>
 1090: 9901 ldr r1, [sp, #4]
 1092: 6808 ldr r0, [r1, #0]
 1094: 7949 ldrb r1, [r1, #5]
 1096: f000 f800 bl 730 <HardwareIo as StartupIo>::gpio>
 109a: e6e9 b.n 670 <SequencerBackend as Backend>::call+0x70>
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
    " 1024: f000 f800 bl 700 <HardwareIo as StartupIo>::device_clock_gates>",
    " 1024: 4770 bx lr\n"
    " 1026: f000 f800 bl 700 <HardwareIo as StartupIo>::device_clock_gates>",
)

BRANCHED_DISPATCH = CONNECTED_DISPATCH.replace(
    " 1024: f000 f800 bl 700 <HardwareIo as StartupIo>::device_clock_gates>",
    " 1024: e000 b.n 102a <unrelated>\n"
    " 1026: f000 f800 bl 700 <HardwareIo as StartupIo>::device_clock_gates>",
)

BACKEND_POST_CALL_SELF_LOOP = CONNECTED_DISPATCH.replace(
    " 1028: e722 b.n 670 <SequencerBackend as Backend>::call+0x70>",
    " 1028: e7fe b.n 1028 <SequencerBackend as Backend>::call+0x48>",
)

BACKEND_CROSS_CASE_CONTINUATION = CONNECTED_DISPATCH.replace(
    " 1028: e722 b.n 670 <SequencerBackend as Backend>::call+0x70>",
    " 1028: e702 b.n 1030 <SequencerBackend as Backend>::call+0x50>",
)


class StartupWitnessCheckerTests(unittest.TestCase):
    def test_accepts_immediate_systick_return_at_symbol_entry(self):
        validate_systick_handler(SYSTICK_RETURN, 0x100)

    def test_accepts_thumb_state_symbol_address(self):
        validate_systick_handler(SYSTICK_RETURN, 0x101)

    def test_rejects_systick_self_loop_before_retained_return(self):
        with self.assertRaisesRegex(WitnessError, "SysTick_Handler.*begin with bx lr"):
            validate_systick_handler(SYSTICK_SELF_LOOP, 0x100)

    def test_rejects_systick_instruction_before_retained_return(self):
        with self.assertRaisesRegex(WitnessError, "SysTick_Handler.*begin with bx lr"):
            validate_systick_handler(SYSTICK_OTHER_BEFORE_RETURN, 0x100)

    def test_rejects_return_at_address_other_than_linked_symbol(self):
        with self.assertRaisesRegex(WitnessError, "SysTick_Handler.*begin with bx lr"):
            validate_systick_handler(SYSTICK_RETURN, 0x102)

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

    def test_rejects_backend_case_self_loop_after_hardware_call(self):
        with self.assertRaisesRegex(
            WitnessError, "variant 0.*shared return epilogue"
        ):
            validate_call_path(BACKEND_POST_CALL_SELF_LOOP)

    def test_rejects_backend_case_continuation_into_next_case(self):
        with self.assertRaisesRegex(
            WitnessError, "variant 0.*shared return epilogue"
        ):
            validate_call_path(BACKEND_CROSS_CASE_CONTINUATION)

    def test_accepts_linked_backend_dispatch_to_matching_hardware_methods(self):
        validate_call_path(CONNECTED_DISPATCH)


if __name__ == "__main__":
    unittest.main()
