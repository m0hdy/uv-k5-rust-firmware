import re
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from check_startup_witness import (  # noqa: E402
    WitnessError,
    reachable_control_flow,
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
 104: e7fe b.n 104 <Reset_Handler+0x4>
00000200 <witness_main>:
 200: f000 f800 bl 300 <run_hardware>
 204: e7fe b.n 204 <witness_main+0x4>
00000300 <run_hardware>:
 300: f000 f800 bl 400 <board_startup_adapters::run::<Hardware>>
 304: b004 add sp, #16
 306: bd80 pop {r7, pc}
00000400 <board_startup_adapters::run::<Hardware>>:
 400: f000 f800 bl 500 <board_sequence::run::<Backend>>
 404: 9801 ldr r0, [sp, #4]
 406: 7980 ldrb r0, [r0, #6]
 408: 07c0 lsls r0, r0, #31
 40a: 2800 cmp r0, #0
 40c: d104 bne.n 418
 40e: e7ff b.n 410
 410: a904 add r1, sp, #16
 412: 2000 movs r0, #0
 414: 7008 strb r0, [r1, #0]
 416: e003 b.n 420
 418: a904 add r1, sp, #16
 41a: 2001 movs r0, #1
 41c: 7008 strb r0, [r1, #0]
 41e: e7ff b.n 420
 420: a804 add r0, sp, #16
 422: 7800 ldrb r0, [r0, #0]
 424: b008 add sp, #32
 426: bd80 pop {r7, pc}
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
 560: a802 add r0, sp, #8
 562: 7840 ldrb r0, [r0, #1]
 564: 07c0 lsls r0, r0, #31
 566: 2800 cmp r0, #0
 568: d809 bne.n 57e
 56a: e7ff b.n 56c
 56c: a802 add r0, sp, #8
 56e: 7880 ldrb r0, [r0, #2]
 570: 07c0 lsls r0, r0, #31
 572: 2800 cmp r0, #0
 574: d008 beq.n 588
 576: e002 b.n 57e
 57e: 9800 ldr r0, [sp, #0]
 580: 2108 movs r1, #8
 582: f000 f800 bl 600 <<Backend>::call>
 586: e000 b.n 588
 588: b004 add sp, #16
 58a: bd80 pop {r7, pc}
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
 610: 2804 cmp r0, #4
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
 630: 10b1 .word 0x10b1
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
 10b0: 9801 ldr r0, [sp, #4]
 10b2: 6800 ldr r0, [r0, #0]
 10b4: f000 f800 bl 740 <HardwareIo as StartupIo>::backlight_init_hardware>
 10b8: e6d6 b.n 670 <SequencerBackend as Backend>::call+0x70>
00000700 <<HardwareIo as StartupIo>::device_clock_gates>:
 700: f000 f800 bl 800 <apply_device_clock_gates>
 704: bd80 pop {r7, pc}
00000710 <<HardwareIo as StartupIo>::systick>:
 710: f000 f800 bl 810 <apply_to_hardware>
 714: f000 f800 bl 811 <Result<(), startup_adapter_witness::board_startup_adapters::systick_startup_adapter::InvalidReload>>::map_err>
 718: bd80 pop {r7, pc}
00000720 <<HardwareIo as StartupIo>::portcon>:
 720: f000 f800 bl 820 <apply_portcon_startup_plan>
 724: bd80 pop {r7, pc}
00000730 <<HardwareIo as StartupIo>::gpio>:
 730: f000 f800 bl 830 <apply_gpio_startup_plan>
 734: b00e add sp, #56
 736: bd80 pop {r7, pc}
00000740 <<HardwareIo as StartupIo>::backlight_init_hardware>:
 740: f000 f800 bl 840 <initialize_backlight_hardware>
 744: bd80 pop {r7, pc}
00000840 <initialize_backlight_hardware>:
 840: f000 f800 bl 846 <backlight_startup_executor::apply_with_io>
 844: bd80 pop {r7, pc}
00000846 <backlight_startup_executor::apply_with_io>:
 846: f000 f800 bl 1000 <backlight_startup_executor::BacklightInitPlan>::from_snapshot>
 84a: f000 f800 bl 1100 <backlight_startup_executor::BacklightInitPlan>::writes>
 84e: 9900 ldr r1, [sp, #156]
 850: f000 f800 bl 900 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::read>
 854: 9901 ldr r1, [sp, #8]
 856: 4602 mov r2, r0
 858: 9804 ldr r0, [sp, #16]
 85a: 430a orrs r2, r1
 85c: 9900 ldr r1, [sp, #156]
 85e: f000 f800 bl 910 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::write>
 862: 9900 ldr r1, [sp, #156]
 864: f000 f800 bl 900 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::read>
 868: 9901 ldr r1, [sp, #4]
 86a: 4602 mov r2, r0
 86c: 9804 ldr r0, [sp, #16]
 86e: 438a bics r2, r1
 870: 9900 ldr r1, [sp, #156]
 872: f000 f800 bl 910 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::write>
 876: 9900 ldr r1, [sp, #156]
 878: f000 f800 bl 900 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::read>
 87c: 9901 ldr r1, [sp, #0]
 87e: 4602 mov r2, r0
 880: 9804 ldr r0, [sp, #16]
 882: 430a orrs r2, r1
 884: 9900 ldr r1, [sp, #156]
 886: f000 f800 bl 910 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::write>
 88a: f000 f800 bl 910 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::write>
 88e: 4770 bx lr
00000900 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::read>:
 900: 4608 mov r0, r1
 902: f000 f800 bl 930 <backlight_startup_executor::RegisterAddress>::from_register>
 906: f000 f800 bl a00 <backlight_startup_executor::RegisterAddress>::address>
 90a: 6800 ldr r0, [r0, #0]
 90c: bd80 pop {r7, pc}
00000910 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::write>:
 910: 4608 mov r0, r1
 912: f000 f800 bl 930 <backlight_startup_executor::RegisterAddress>::from_register>
 916: f000 f800 bl a00 <backlight_startup_executor::RegisterAddress>::address>
 91a: 9a01 ldr r2, [sp, #4]
 91c: 6002 str r2, [r0, #0]
 91e: bd80 pop {r7, pc}
00000930 <backlight_startup_executor::RegisterAddress>::from_register>:
 930: a001 add r0, pc, #4 @ (adr r0, 938)
 932: 5840 ldr r0, [r0, r1]
 934: 4687 mov pc, r0
 938: 0951 .word 0x0951
 93c: 0959 .word 0x0959
 940: 0961 .word 0x0961
 944: 0969 .word 0x0969
 948: 0971 .word 0x0971
 950: a901 add r1, sp, #4
 952: 2000 movs r0, #0
 954: 7008 strb r0, [r1, #0]
 958: a901 add r1, sp, #4
 95a: 2001 movs r0, #1
 95c: 7008 strb r0, [r1, #0]
 960: a901 add r1, sp, #4
 962: 2002 movs r0, #2
 964: 7008 strb r0, [r1, #0]
 968: a901 add r1, sp, #4
 96a: 2003 movs r0, #3
 96c: 7008 strb r0, [r1, #0]
 970: a901 add r1, sp, #4
 972: 2004 movs r0, #4
 974: 7008 strb r0, [r1, #0]
00000a00 <backlight_startup_executor::RegisterAddress>::address>:
 a00: a001 add r0, pc, #4 @ (adr r0, a08)
 a02: 5840 ldr r0, [r0, r1]
 a04: 4687 mov pc, r0
 a08: 0a21 .word 0x0a21
 a0c: 0a29 .word 0x0a29
 a10: 0a31 .word 0x0a31
 a14: 0a39 .word 0x0a39
 a18: 0a41 .word 0x0a41
 a20: 480f ldr r0, [pc, #60] @ (a60 <literal>)
 a22: bd80 pop {r7, pc}
 a28: 480e ldr r0, [pc, #56] @ (a64 <literal>)
 a2a: bd80 pop {r7, pc}
 a30: 480d ldr r0, [pc, #52] @ (a68 <literal>)
 a32: bd80 pop {r7, pc}
 a38: 480c ldr r0, [pc, #48] @ (a6c <literal>)
 a3a: bd80 pop {r7, pc}
 a40: 480b ldr r0, [pc, #44] @ (a70 <literal>)
 a42: bd80 pop {r7, pc}
 a60: 400b4008 .word 0x400b4008
 a64: 400b401c .word 0x400b401c
 a68: 400b0008 .word 0x400b0008
 a6c: 400b4004 .word 0x400b4004
 a70: 400b4000 .word 0x400b4000
00001000 <backlight_startup_witness::backlight_startup_executor::BacklightInitPlan>::from_snapshot>:
 1000: 2317 movs r3, #23
 1002: 045b lsls r3, r3, #17
 1004: 4319 orrs r1, r3
 1006: 240f movs r4, #15
 1008: 0625 lsls r5, r4, #24
 100a: 43aa bics r2, r5
 100c: 061b lsls r3, r3, #24
 100e: 431a orrs r2, r3
 1010: 2105 movs r1, #5
 1012: 4801 ldr r0, [pc, #4] @ (1018 <literal>)
 1014: 4901 ldr r1, [pc, #4] @ (101c <literal>)
 1016: e002 b.n 1020 <BacklightInitPlan>::from_snapshot+0x20>
 1018: 000003ff .word 0x000003ff
 101c: 01010000 .word 0x01010000
 1020: bd80 pop {r7, pc}
00001100 <backlight_startup_executor::BacklightInitPlan>::writes>:
 1100: 2106 movs r1, #6
 1102: f000 f800 bl 1110 <backlight_startup_executor::BacklightWrite>::iter>
 1106: bd80 pop {r7, pc}
00001110 <backlight_startup_executor::BacklightWrite>::iter>:
 1110: bd80 pop {r7, pc}
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
    " 610: 2804 cmp r0, #4\n 612: d81d bhi.n 650",
    " 610: 4280 cmp r0, r0\n 612: d01d beq.n 650",
)

UNSUPPORTED_BACKEND_CONDITIONAL = CONNECTED_DISPATCH.replace(
    " 610: 2804 cmp r0, #4\n 612: d81d bhi.n 650",
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

HARDWARE_WRAPPER_POST_CALL_SELF_LOOP = CONNECTED_DISPATCH.replace(
    " 704: bd80 pop {r7, pc}",
    " 704: e7fe b.n 704\n 706: bd80 pop {r7, pc}",
)

RUN_HARDWARE_POST_CALL_SELF_LOOP = CONNECTED_DISPATCH.replace(
    " 304: b004 add sp, #16",
    " 304: e7fe b.n 304\n 306: b004 add sp, #16",
)

ADAPTER_RUN_ERROR_SELF_LOOP = CONNECTED_DISPATCH.replace(
    " 41e: e7ff b.n 420",
    " 41e: e7fe b.n 41e",
)

ADAPTER_RUN_SUCCESS_SELF_LOOP = CONNECTED_DISPATCH.replace(
    " 416: e003 b.n 420",
    " 416: e7fe b.n 416",
)

SEQUENCER_FM_POST_CALL_SELF_LOOP = CONNECTED_DISPATCH.replace(
    " 558: e002 b.n 560 <board_sequence::run+0x60>",
    " 558: e7fe b.n 558\n 55a: e002 b.n 560 <board_sequence::run+0x60>",
)

SEQUENCER_CRC_POST_CALL_SELF_LOOP = CONNECTED_DISPATCH.replace(
    " 586: e000 b.n 588",
    " 586: e7fe b.n 586",
)

SEQUENCER_CRC_SKIP_SELF_LOOP = CONNECTED_DISPATCH.replace(
    " 574: d008 beq.n 588",
    " 574: e7fe b.n 574",
)

BACKLIGHT_DISPATCH_UNREACHABLE = CONNECTED_DISPATCH.replace(
    " 10b0: 9801 ldr r0, [sp, #4]",
    " 10b0: 4770 bx lr",
)

BACKLIGHT_WRONG_REGISTER_ADDRESS = CONNECTED_DISPATCH.replace(
    " a68: 400b0008 .word 0x400b0008",
    " a68: 400b000c .word 0x400b000c",
)

BACKLIGHT_SWAPPED_REGISTER_ADDRESSES = CONNECTED_DISPATCH.replace(
    " a60: 400b4008 .word 0x400b4008\n a64: 400b401c .word 0x400b401c",
    " a60: 400b401c .word 0x400b401c\n a64: 400b4008 .word 0x400b4008",
)

BACKLIGHT_WRONG_PLAN_OPERAND = CONNECTED_DISPATCH.replace(
    " 1000: 2317 movs r3, #23",
    " 1000: 2316 movs r3, #22",
)

BACKLIGHT_MISSING_FRESH_READ = CONNECTED_DISPATCH.replace(
    " 864: f000 f800 bl 900 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::read>",
    " 864: f000 f800 bl 900 <unrelated_read>",
)

BACKLIGHT_WRONG_RMW_ORDER = CONNECTED_DISPATCH.replace(
    " 85a: 430a orrs r2, r1",
    " 85a: 438a bics r2, r1",
)

BACKLIGHT_EXECUTOR_EARLY_RETURN = CONNECTED_DISPATCH
_apply_header = "00000846 <backlight_startup_executor::apply_with_io>:\n"
_apply_start = BACKLIGHT_EXECUTOR_EARLY_RETURN.index(_apply_header) + len(_apply_header)
_apply_end = BACKLIGHT_EXECUTOR_EARLY_RETURN.index(
    "00000900 <<backlight_startup_executor::VolatileRegisterIo as"
)


def _shift_apply_instructions(disassembly, first_address):
    start = disassembly.index(_apply_header) + len(_apply_header)
    end = disassembly.index(
        "00000900 <<backlight_startup_executor::VolatileRegisterIo as"
    )
    body = disassembly[start:end]

    def shift(match):
        address = int(match.group(2), 16)
        if address < first_address:
            return match.group(0)
        return (
            f"{match.group(1)}"
            f"{address + 2:0{len(match.group(2))}x}:"
        )

    return disassembly[:start] + re.sub(r"(?m)^(\s*)([0-9a-fA-F]+):", shift, body) + disassembly[end:]


_shifted_apply = re.sub(
    r"(?m)^(\s*)([0-9a-fA-F]+):",
    lambda match: (
        f"{match.group(1)}"
        f"{int(match.group(2), 16) + 2:0{len(match.group(2))}x}:"
    ),
    BACKLIGHT_EXECUTOR_EARLY_RETURN[_apply_start:_apply_end],
)
BACKLIGHT_EXECUTOR_EARLY_RETURN = (
    BACKLIGHT_EXECUTOR_EARLY_RETURN[:_apply_start]
    + " 846: 4770 bx lr\n"
    + _shifted_apply
    + BACKLIGHT_EXECUTOR_EARLY_RETURN[_apply_end:]
)

BACKLIGHT_EXECUTOR_BRANCH_SKIPS_PLAN = CONNECTED_DISPATCH.replace(
    " 846: f000 f800 bl 1000 <backlight_startup_executor::BacklightInitPlan>::from_snapshot>",
    " 846: e000 b.n 84a <backlight_startup_executor::apply_with_io+0x4>",
)

BACKLIGHT_EXECUTOR_CONDITIONAL_SKIPS_PLAN = CONNECTED_DISPATCH.replace(
    " 846: f000 f800 bl 1000 <backlight_startup_executor::BacklightInitPlan>::from_snapshot>",
    " 846: d000 beq.n 84a <backlight_startup_executor::apply_with_io+0x4>\n"
    " 848: 46c0 nop",
)

BACKLIGHT_EXECUTOR_PLAN_BYPASS_REJOIN = _shift_apply_instructions(
    CONNECTED_DISPATCH, 0x84A
).replace(
    " 846: f000 f800 bl 1000 <backlight_startup_executor::BacklightInitPlan>::from_snapshot>",
    " 846: d001 beq.n 84c <backlight_startup_executor::apply_with_io+0x6>\n"
    " 848: f000 f800 bl 1000 <backlight_startup_executor::BacklightInitPlan>::from_snapshot>",
)

BACKLIGHT_EXECUTOR_RETURN_BYPASSES_PLAN = _shift_apply_instructions(
    CONNECTED_DISPATCH, 0x84A
).replace(
    " 846: f000 f800 bl 1000 <backlight_startup_executor::BacklightInitPlan>::from_snapshot>",
    " 846: d023 beq.n 890 <backlight_startup_executor::apply_with_io+0x4a>\n"
    " 848: f000 f800 bl 1000 <backlight_startup_executor::BacklightInitPlan>::from_snapshot>",
)

BACKLIGHT_EXECUTOR_UNSUPPORTED_BRANCH = CONNECTED_DISPATCH.replace(
    " 846: f000 f800 bl 1000 <backlight_startup_executor::BacklightInitPlan>::from_snapshot>",
    " 846: 4687 mov pc, r0",
)

BACKLIGHT_EXECUTOR_RMW_BYPASS_REJOIN = _shift_apply_instructions(
    CONNECTED_DISPATCH, 0x852
).replace(
    " 850: f000 f800 bl 900 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::read>",
    " 850: d006 beq.n 860 <backlight_startup_executor::apply_with_io+0x1a>\n"
    " 852: f000 f800 bl 900 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::read>",
)

BACKLIGHT_PLAN_EARLY_RETURN = CONNECTED_DISPATCH.replace(
    " 1000: 2317 movs r3, #23",
    " 1000: 4770 bx lr",
)

BACKLIGHT_REGISTER_READ_EARLY_RETURN = CONNECTED_DISPATCH.replace(
    " 900: 4608 mov r0, r1",
    " 900: 4770 bx lr",
)

BACKLIGHT_INDIRECT_BRANCH_BYPASSES_SETUP = """
00000a00 <register_mapping>:
 a00: d001 beq.n a06 <register_mapping+0x6>
 a02: a001 add r0, pc, #4 @ (adr r0, a08)
 a04: 5840 ldr r0, [r0, r1]
 a06: 4687 mov pc, r0
 a08: 0a11 .word 0x00000a11
 a0c: 0a13 .word 0x00000a13
 a10: 2000 movs r0, #0
 a12: 4770 bx lr
"""

TRAP_BEFORE_RETURN = """
00000100 <trap_path>:
 100: de00 udf #0
 102: 4770 bx lr
"""


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

    def test_rejects_hardware_wrapper_self_loop_after_adapter_call(self):
        with self.assertRaisesRegex(
            WitnessError, "target device_clock_gates.*non-returning call continuation"
        ):
            validate_call_path(HARDWARE_WRAPPER_POST_CALL_SELF_LOOP)

    def test_rejects_run_hardware_self_loop_after_adapter_call(self):
        with self.assertRaisesRegex(
            WitnessError, "run_hardware.*non-returning call continuation"
        ):
            validate_call_path(RUN_HARDWARE_POST_CALL_SELF_LOOP)

    def test_rejects_adapter_error_result_self_loop(self):
        with self.assertRaisesRegex(
            WitnessError, "board startup adapter result paths do not reach"
        ):
            validate_call_path(ADAPTER_RUN_ERROR_SELF_LOOP)

    def test_rejects_adapter_success_result_self_loop(self):
        with self.assertRaisesRegex(
            WitnessError, "board startup adapter result paths do not reach"
        ):
            validate_call_path(ADAPTER_RUN_SUCCESS_SELF_LOOP)

    def test_rejects_fm_optional_call_self_loop(self):
        with self.assertRaisesRegex(
            WitnessError, "variant-seven call does not return to the next stage"
        ):
            validate_call_path(SEQUENCER_FM_POST_CALL_SELF_LOOP)

    def test_rejects_crc_optional_call_self_loop(self):
        with self.assertRaisesRegex(
            WitnessError, "optional CRC paths do not reach"
        ):
            validate_call_path(SEQUENCER_CRC_POST_CALL_SELF_LOOP)

    def test_rejects_crc_optional_skip_self_loop(self):
        with self.assertRaisesRegex(
            WitnessError, "optional CRC path has an unexpected continuation"
        ):
            validate_call_path(SEQUENCER_CRC_SKIP_SELF_LOOP)

    def test_accepts_linked_backend_dispatch_to_matching_hardware_methods(self):
        validate_call_path(CONNECTED_DISPATCH)

    def test_rejects_backlight_executor_evidence_after_address_preserving_early_return(self):
        self.assertIn("00000846 <backlight_startup_executor::apply_with_io>", BACKLIGHT_EXECUTOR_EARLY_RETURN)
        self.assertIn(" 846: 4770 bx lr", BACKLIGHT_EXECUTOR_EARLY_RETURN)
        self.assertIn(" 848: f000 f800 bl 1000", BACKLIGHT_EXECUTOR_EARLY_RETURN)

        with self.assertRaisesRegex(WitnessError, "backlight executor.*initialization plan"):
            validate_call_path(BACKLIGHT_EXECUTOR_EARLY_RETURN)

    def test_rejects_backlight_executor_branch_that_skips_plan_creation(self):
        with self.assertRaisesRegex(WitnessError, "backlight executor.*initialization plan"):
            validate_call_path(BACKLIGHT_EXECUTOR_BRANCH_SKIPS_PLAN)

    def test_rejects_backlight_executor_conditional_branch_that_skips_plan_creation(self):
        with self.assertRaisesRegex(WitnessError, "backlight executor.*initialization plan"):
            validate_call_path(BACKLIGHT_EXECUTOR_CONDITIONAL_SKIPS_PLAN)

    def test_rejects_backlight_executor_branch_that_bypasses_plan_call_and_rejoins(self):
        with self.assertRaisesRegex(WitnessError, "backlight executor.*initialization plan"):
            validate_call_path(BACKLIGHT_EXECUTOR_PLAN_BYPASS_REJOIN)

    def test_rejects_backlight_executor_branch_that_returns_before_plan(self):
        with self.assertRaisesRegex(WitnessError, "backlight executor.*initialization plan"):
            validate_call_path(BACKLIGHT_EXECUTOR_RETURN_BYPASSES_PLAN)

    def test_rejects_backlight_rmw_branch_that_bypasses_read_and_rejoins(self):
        with self.assertRaisesRegex(WitnessError, "ordered fresh RMW register evidence"):
            validate_call_path(BACKLIGHT_EXECUTOR_RMW_BYPASS_REJOIN)

    def test_rejects_unsupported_backlight_executor_indirect_branch(self):
        with self.assertRaisesRegex(WitnessError, "backlight executor.*unsupported indirect branch"):
            validate_call_path(BACKLIGHT_EXECUTOR_UNSUPPORTED_BRANCH)

    def test_rejects_backlight_plan_evidence_after_early_return(self):
        with self.assertRaisesRegex(WitnessError, "backlight plan operands"):
            validate_call_path(BACKLIGHT_PLAN_EARLY_RETURN)

    def test_rejects_register_mapping_evidence_after_early_return(self):
        with self.assertRaisesRegex(
            WitnessError, "backlight register read does not preserve its mapped register"
        ):
            validate_call_path(BACKLIGHT_REGISTER_READ_EARLY_RETURN)

    def test_rejects_indirect_branch_that_bypasses_its_table_setup(self):
        with self.assertRaisesRegex(
            WitnessError, "indirect branch that bypasses its table setup"
        ):
            reachable_control_flow(
                BACKLIGHT_INDIRECT_BRANCH_BYPASSES_SETUP, "backlight register mapping"
            )

    def test_rejects_evidence_after_a_trap_instruction(self):
        with self.assertRaisesRegex(WitnessError, "unsupported trap instruction"):
            reachable_control_flow(TRAP_BEFORE_RETURN, "backlight register mapping")

    def test_rejects_unreachable_backlight_hardware_call(self):
        with self.assertRaisesRegex(WitnessError, "variant 4.*unexpected continuation"):
            validate_call_path(BACKLIGHT_DISPATCH_UNREACHABLE)

    def test_rejects_wrong_backlight_register_address(self):
        with self.assertRaisesRegex(WitnessError, "backlight register address evidence"):
            validate_call_path(BACKLIGHT_WRONG_REGISTER_ADDRESS)

    def test_rejects_swapped_backlight_register_addresses(self):
        with self.assertRaisesRegex(
            WitnessError, "backlight register addresses do not match their register variants"
        ):
            validate_call_path(BACKLIGHT_SWAPPED_REGISTER_ADDRESSES)

    def test_rejects_wrong_backlight_plan_operand(self):
        with self.assertRaisesRegex(WitnessError, "backlight plan operands"):
            validate_call_path(BACKLIGHT_WRONG_PLAN_OPERAND)

    def test_rejects_missing_backlight_fresh_read(self):
        with self.assertRaisesRegex(WitnessError, "ordered fresh RMW register evidence"):
            validate_call_path(BACKLIGHT_MISSING_FRESH_READ)

    def test_rejects_wrong_backlight_rmw_order(self):
        with self.assertRaisesRegex(WitnessError, "ordered fresh RMW register evidence"):
            validate_call_path(BACKLIGHT_WRONG_RMW_ORDER)

    def test_literal_load_accepts_gnu_annotation_styles(self):
        for marker in ("@", ";"):
            with self.subTest(marker=marker):
                disassembly = CONNECTED_DISPATCH.replace(
                    " 1034: 490d ldr r1, [pc, #52]",
                    f" 1034:\t490d      \tldr\tr1, [pc, #52]\t{marker} (106c <literal>)",
                )
                validate_call_path(disassembly)

    def test_literal_load_rejects_wrong_operand_with_annotation(self):
        for marker in ("@", ";"):
            with self.subTest(marker=marker):
                disassembly = CONNECTED_DISPATCH.replace(
                    " 1034: 490d ldr r1, [pc, #52]",
                    f" 1034:\t490d      \tldr\tr2, [pc, #52]\t{marker} (106c <literal>)",
                )
                with self.assertRaisesRegex(WitnessError, "variant 1.*continuation"):
                    validate_call_path(disassembly)


if __name__ == "__main__":
    unittest.main()
