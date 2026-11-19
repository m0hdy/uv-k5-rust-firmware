import re
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from check_startup_witness import (  # noqa: E402
    WitnessError,
    reachable_control_flow,
    validate,
    validate_backlight_initializer_path,
    validate_backlight_register_io_path,
    validate_call_path,
    validate_st7565_backend_case,
    validate_st7565_executor_path,
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
 300: b5f0 push {r4, r5, r6, r7, lr}
 302: af03 add r7, sp, #12
 304: b087 sub sp, #28
 306: f000 f800 bl 400 <board_startup_adapters::run::<Hardware>>
 30a: b007 add sp, #28
 30c: bdf0 pop {r4, r5, r6, r7, pc}
00000400 <board_startup_adapters::run::<Hardware>>:
 400: f000 f800 bl 500 <board_sequence::run::<Backend>>
 404: 980b ldr r0, [sp, #44]
 406: b00d add sp, #52
 408: bdf0 pop {r4, r5, r6, r7, pc}
00000500 <board_sequence::run::<Backend>>:
 500: 2100 movs r1, #0
 502: f000 f800 bl 5f6 <startup::SequencerBackend<Hardware> as startup::board_sequence::Backend>::call>
 506: 9800 ldr r0, [sp, #0]
 508: 2101 movs r1, #1
 50a: f000 f800 bl 5f6 <startup::SequencerBackend<Hardware> as startup::board_sequence::Backend>::call>
 50e: 9800 ldr r0, [sp, #0]
 510: 2102 movs r1, #2
 512: f000 f800 bl 5f6 <startup::SequencerBackend<Hardware> as startup::board_sequence::Backend>::call>
 516: 9800 ldr r0, [sp, #0]
 518: 2103 movs r1, #3
 51a: f000 f800 bl 5f6 <startup::SequencerBackend<Hardware> as startup::board_sequence::Backend>::call>
 51e: 9800 ldr r0, [sp, #0]
 520: 2104 movs r1, #4
 522: f000 f800 bl 5f6 <startup::SequencerBackend<Hardware> as startup::board_sequence::Backend>::call>
 526: 9800 ldr r0, [sp, #0]
 528: 2105 movs r1, #5
 52a: f000 f800 bl 5f6 <startup::SequencerBackend<Hardware> as startup::board_sequence::Backend>::call>
 52e: 9800 ldr r0, [sp, #0]
 530: 2106 movs r1, #6
 532: f000 f800 bl 5f6 <startup::SequencerBackend<Hardware> as startup::board_sequence::Backend>::call>
 536: 9801 ldr r0, [sp, #4]
 538: 7800 ldrb r0, [r0, #0]
 53a: 07c0 lsls r0, r0, #31
 53c: 2800 cmp r0, #0
 53e: d807 bne.n 550 <board_sequence::run+0x50>
 540: e00e b.n 560 <board_sequence::run+0x60>
 550: 9800 ldr r0, [sp, #0]
 552: 2107 movs r1, #7
 554: f000 f800 bl 5f6 <startup::SequencerBackend<Hardware> as startup::board_sequence::Backend>::call>
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
 582: f000 f800 bl 5f6 <startup::SequencerBackend<Hardware> as startup::board_sequence::Backend>::call>
 586: e000 b.n 588
 588: b004 add sp, #16
 58a: bd80 pop {r7, pc}
000005f6 <startup::SequencerBackend<Hardware> as startup::board_sequence::Backend>::call>:
 5f6: b580 push {r7, lr}
 5f8: af00 add r7, sp, #0
 5fa: b086 sub sp, #24
 5fc: 9002 str r0, [sp, #8]
 5fe: 9103 str r1, [sp, #12]
 600: 7980 ldrb r0, [r0, #8]
 602: 07c0 lsls r0, r0, #31
 604: 2800 cmp r0, #0
 606: d112 bne.n 670 <SequencerBackend as Backend>::call+0x70
 608: e7ff b.n 60a <SequencerBackend as Backend>::call+0xa
 60a: 9803 ldr r0, [sp, #12]
 60c: b2c0 uxtb r0, r0
 60e: 9001 str r0, [sp, #4]
 610: 9801 ldr r0, [sp, #4]
 612: 0081 lsls r1, r0, #2
 614: a002 add r0, pc, #8 @ (adr r0, 620 <SequencerBackend as Backend>::call+0x20)
 616: 5840 ldr r0, [r0, r1]
 618: 4687 mov pc, r0
 620: 1021 .word 0x1021
 624: 1031 .word 0x1031
 628: 1071 .word 0x1071
 62c: 1091 .word 0x1091
 630: 10b1 .word 0x10b1
 634: 10c1 .word 0x10c1
 638: 1379 .word 0x1379
 63c: 0651 .word 0x0651
 640: 13b1 .word 0x13b1
 650: 9903 ldr r1, [sp, #12]
 652: 9802 ldr r0, [sp, #8]
 654: 6800 ldr r0, [r0, #0]
 656: f000 f8f3 bl 1300 <<startup_adapter_witness::board_startup_adapters::HardwareIo as startup_adapter_witness::board_startup_adapters::StartupIo>::placeholder>
 65a: e709 b.n 670 <SequencerBackend as Backend>::call+0x70
 670: b006 add sp, #24
 672: bd80 pop {r7, pc}
 1020: 9802 ldr r0, [sp, #8]
 1022: 6800 ldr r0, [r0, #0]
 1024: f000 f800 bl 700 <HardwareIo as StartupIo>::device_clock_gates>
 1028: e722 b.n 670 <SequencerBackend as Backend>::call+0x70>
 1030: 9802 ldr r0, [sp, #8]
 1032: 6800 ldr r0, [r0, #0]
 1034: 490d ldr r1, [pc, #52]
 1036: f000 f800 bl 710 <HardwareIo as StartupIo>::systick>
 103a: 4601 mov r1, r0
 103c: a804 add r0, sp, #16
 103e: 7001 strb r1, [r0, #0]
 1040: f000 f800 bl 1310 <<core::result::Result<(), startup_adapter_witness::board_startup_adapters::StartupError>>::is_err>
 1044: 2800 cmp r0, #0
 1046: d10b bne.n 1060 <SequencerBackend as Backend>::call+0x80>
 1048: e712 b.n 670 <SequencerBackend as Backend>::call+0x70>
 1060: 9902 ldr r1, [sp, #8]
 1062: 2001 movs r0, #1
 1064: 7208 strb r0, [r1, #8]
 1066: 20ff movs r0, #255
 1068: 7328 strb r0, [r1, #24]
 106a: e701 b.n 670 <SequencerBackend as Backend>::call+0x70>
 1070: 9902 ldr r1, [sp, #8]
 1072: 6808 ldr r0, [r1, #0]
 1074: 7949 ldrb r1, [r1, #4]
 1076: f000 f800 bl 720 <HardwareIo as StartupIo>::portcon>
 107a: e6f9 b.n 670 <SequencerBackend as Backend>::call+0x70>
 1090: 9902 ldr r1, [sp, #8]
 1092: 6808 ldr r0, [r1, #0]
 1094: 7949 ldrb r1, [r1, #5]
 1096: f000 f800 bl 730 <HardwareIo as StartupIo>::gpio>
 109a: e6e9 b.n 670 <SequencerBackend as Backend>::call+0x70>
 10b0: 9802 ldr r0, [sp, #8]
 10b2: 6800 ldr r0, [r0, #0]
 10b4: f000 f800 bl 740 <HardwareIo as StartupIo>::backlight_init_hardware>
 10b8: e6d6 b.n 670 <SequencerBackend as Backend>::call+0x70>
 10c0: 9902 ldr r1, [sp, #8]
 10c2: 6808 ldr r0, [r1, #0]
 10c4: 88c9 ldrh r1, [r1, #6]
 10c6: f000 f800 bl 1320 <HardwareIo as StartupIo>::adc_init>
 10ca: e6d6 b.n 670 <SequencerBackend as Backend>::call+0x70>
 1378: 9b02 ldr r3, [sp, #8]
 137a: 6818 ldr r0, [r3, #0]
 137c: 68d9 ldr r1, [r3, #12]
 137e: 691a ldr r2, [r3, #16]
 1380: 695b ldr r3, [r3, #20]
 1382: f000 f800 bl 1400 <HardwareIo as StartupIo>::st7565_init>
 1386: 4601 mov r1, r0
 1388: a805 add r0, sp, #20
 138a: 7001 strb r1, [r0, #0]
 138c: 7800 ldrb r0, [r0, #0]
 138e: 28ff cmp r0, #255
 1390: d102 bne.n 1398
 1392: e66d b.n 670 <SequencerBackend as Backend>::call+0x70>
 1398: 9902 ldr r1, [sp, #8]
 139a: 9805 ldr r0, [sp, #20]
 139c: 2201 movs r2, #1
 139e: 720a strb r2, [r1, #8]
 13a0: 7608 strb r0, [r1, #24]
 13a2: e665 b.n 670 <SequencerBackend as Backend>::call+0x70>
 13b0: 9802 ldr r0, [sp, #8]
 13b2: 6800 ldr r0, [r0, #0]
 13b4: f000 f800 bl 1500 <HardwareIo as StartupIo>::crc_init>
 13b8: e000 b.n 670
00000700 <<HardwareIo as StartupIo>::device_clock_gates>:
 700: f000 f800 bl 800 <apply_device_clock_gates>
 704: bd80 pop {r7, pc}
00000710 <<HardwareIo as StartupIo>::systick>:
 710: f000 f800 bl 810 <systick_startup_adapter::apply_to_hardware>
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
00001320 <<HardwareIo as StartupIo>::adc_init>:
 1320: f000 f800 bl 1330 <adc_startup_adapter::physical::target::apply_to_hardware>
 1324: bd80 pop {r7, pc}
00001400 <<HardwareIo as StartupIo>::st7565_init>:
 1400: b580 push {r7, lr}
 1402: af00 add r7, sp, #0
 1404: b082 sub sp, #8
 1406: 9300 str r3, [sp, #0]
 1408: 4610 mov r0, r2
 140a: 9a00 ldr r2, [sp, #0]
 140c: 9001 str r0, [sp, #4]
 140e: 4608 mov r0, r1
 1410: 9901 ldr r1, [sp, #4]
 1412: f000 f800 bl 1b00 <target_spi0::startup_mmio>
 1416: b002 add sp, #8
 1418: bd80 pop {r7, pc}
00001b00 <target_spi0::startup_mmio>:
 1b00: b580 push {r7, lr}
 1b02: af00 add r7, sp, #0
 1b04: b082 sub sp, #8
 1b06: 4613 mov r3, r2
 1b08: 460a mov r2, r1
 1b0a: 4601 mov r1, r0
 1b0c: a801 add r0, sp, #4
 1b0e: f000 f800 bl 1420 <target_spi0::startup_with_io::<Registers, Delay>>
 1b12: b002 add sp, #8
 1b14: bd80 pop {r7, pc}
00001420 <target_spi0::startup_with_io::<Registers, Delay>>:
 1420: 4770 bx lr
00001330 <adc_startup_adapter::physical::target::apply_to_hardware>:
 1330: b580 push {r7, lr}
 1332: af00 add r7, sp, #0
 1334: b082 sub sp, #8
 1336: 4601 mov r1, r0
 1338: a801 add r0, sp, #4
 133a: f000 f800 bl 1348 <adc_startup_adapter::physical::apply_with_words::<Mock>>
 133e: b002 add sp, #8
 1340: bd80 pop {r7, pc}
00001348 <adc_startup_adapter::physical::apply_with_words::<Mock>>:
 1348: b580 push {r7, lr}
 134a: af00 add r7, sp, #0
 134c: b082 sub sp, #8
 134e: 9001 str r0, [sp, #4]
 1350: a801 add r0, sp, #4
 1352: f000 f800 bl 1360 <adc_startup_adapter::apply_with_io::<Mock>>
 1356: b002 add sp, #8
 1358: bd80 pop {r7, pc}
00001360 <adc_startup_adapter::apply_with_io::<Mock>>:
 1360: 4770 bx lr
00001368 <adc_startup_adapter::noop>:
 1368: 4770 bx lr
00000800 <apply_device_clock_gates>:
 800: 4770 bx lr
00000810 <systick_startup_adapter::apply_to_hardware>:
 810: 4770 bx lr
00000811 <Result<(), startup_adapter_witness::board_startup_adapters::systick_startup_adapter::InvalidReload>>::map_err>:
 811: bd80 pop {r7, pc}
00000820 <apply_portcon_startup_plan>:
 820: 4770 bx lr
00000830 <apply_gpio_startup_plan>:
 830: 4770 bx lr
00000840 <initialize_backlight_hardware>:
 840: f000 f800 bl 846 <backlight_startup_executor::apply_with_io>
 844: bd80 pop {r7, pc}
00001300 <<startup_adapter_witness::board_startup_adapters::HardwareIo as startup_adapter_witness::board_startup_adapters::StartupIo>::placeholder>:
 1300: 4770 bx lr
00001310 <<core::result::Result<(), startup_adapter_witness::board_startup_adapters::StartupError>>::is_err>:
 1310: bd80 pop {r7, pc}
00000846 <backlight_startup_executor::apply_with_io>:
 846: b580 push {r7, lr}
 848: af00 add r7, sp, #0
 84a: b0a8 sub sp, #160
 84c: 9004 str r0, [sp, #16]
 84e: a806 add r0, sp, #24
 850: 9005 str r0, [sp, #20]
 852: 2200 movs r2, #0
 854: 4611 mov r1, r2
 856: f000 f800 bl 1000 <backlight_startup_executor::BacklightInitPlan>::from_snapshot>
 85a: 9805 ldr r0, [sp, #20]
 85c: f000 f800 bl 1100 <backlight_startup_executor::BacklightInitPlan>::writes>
 860: f000 f800 bl 1180 <core::iter::adapters::copied::Copied<core::slice::iter::Iter<backlight_plan::BacklightWrite>> as core::iter::traits::collect::IntoIterator>::into_iter>
 864: 901e str r0, [sp, #120]
 866: 911f str r1, [sp, #124]
 868: e7ff b.n 86a
 86a: a820 add r0, sp, #128
 86c: a91e add r1, sp, #120
 86e: f000 f800 bl 1190 <core::iter::adapters::copied::Copied<core::slice::iter::Iter<backlight_plan::BacklightWrite>> as core::iter::traits::iterator::Iterator>::next>
 872: 9820 ldr r0, [sp, #128]
 874: 1c40 adds r0, r0, #1
 876: 2800 cmp r0, #0
 878: d018 beq.n 8ac
 87a: e7ff b.n 87c
 87c: 9823 ldr r0, [sp, #140]
 87e: 9027 str r0, [sp, #156]
 880: 9822 ldr r0, [sp, #136]
 882: 9026 str r0, [sp, #152]
 884: 9821 ldr r0, [sp, #132]
 886: 9025 str r0, [sp, #148]
 888: 9820 ldr r0, [sp, #128]
 88a: 9024 str r0, [sp, #144]
 88c: 9824 ldr r0, [sp, #144]
 88e: 9003 str r0, [sp, #12]
 890: 9803 ldr r0, [sp, #12]
 892: 0081 lsls r1, r0, #2
 894: a001 add r0, pc, #4 @ (adr r0, 89c)
 896: 5840 ldr r0, [r0, r1]
 898: 4687 mov pc, r0
 89c: 08b1 .word 0x08b1
 8a0: 08cd .word 0x08cd
 8a4: 08e9 .word 0x08e9
 8a8: 0905 .word 0x0905
 8ac: b028 add sp, #160
 8ae: bd80 pop {r7, pc}
 8b0: 9804 ldr r0, [sp, #16]
 8b2: 9925 ldr r1, [sp, #148]
 8b4: 9102 str r1, [sp, #8]
 8b6: 9927 ldr r1, [sp, #156]
 8b8: f000 f800 bl 920 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::read>
 8bc: 9902 ldr r1, [sp, #8]
 8be: 4602 mov r2, r0
 8c0: 9804 ldr r0, [sp, #16]
 8c2: 430a orrs r2, r1
 8c4: 9927 ldr r1, [sp, #156]
 8c6: f000 f800 bl 2000 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::write>
 8ca: e7ce b.n 86a
 8cc: 9804 ldr r0, [sp, #16]
 8ce: 9925 ldr r1, [sp, #148]
 8d0: 9101 str r1, [sp, #4]
 8d2: 9927 ldr r1, [sp, #156]
 8d4: f000 f800 bl 920 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::read>
 8d8: 9901 ldr r1, [sp, #4]
 8da: 4602 mov r2, r0
 8dc: 9804 ldr r0, [sp, #16]
 8de: 438a bics r2, r1
 8e0: 9927 ldr r1, [sp, #156]
 8e2: f000 f800 bl 2000 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::write>
 8e6: e7c0 b.n 86a
 8e8: 9804 ldr r0, [sp, #16]
 8ea: 9925 ldr r1, [sp, #148]
 8ec: 9100 str r1, [sp, #0]
 8ee: 9927 ldr r1, [sp, #156]
 8f0: f000 f800 bl 920 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::read>
 8f4: 9900 ldr r1, [sp, #0]
 8f6: 4602 mov r2, r0
 8f8: 9804 ldr r0, [sp, #16]
 8fa: 430a orrs r2, r1
 8fc: 9927 ldr r1, [sp, #156]
 8fe: f000 f800 bl 2000 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::write>
 902: e7b2 b.n 86a
 904: 9804 ldr r0, [sp, #16]
 906: 9a26 ldr r2, [sp, #152]
 908: 9927 ldr r1, [sp, #156]
 90a: f000 f800 bl 2000 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::write>
 90e: e7ac b.n 86a
00000920 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::read>:
 920: b580 push {r7, lr}
 922: af00 add r7, sp, #0
 924: 4608 mov r0, r1
 926: f000 f800 bl 942 <backlight_startup_executor::RegisterAddress>::from_register>
 92a: f000 f800 bl 9f2 <backlight_startup_executor::RegisterAddress>::address>
 92e: 6800 ldr r0, [r0, #0]
 930: bd80 pop {r7, pc}
00002000 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::write>:
 2000: b580 push {r7, lr}
 2002: af00 add r7, sp, #0
 2004: b082 sub sp, #8
 2006: 9201 str r2, [sp, #4]
 2008: 4608 mov r0, r1
 200a: f000 f800 bl 942 <backlight_startup_executor::RegisterAddress>::from_register>
 200e: f000 f800 bl 9f2 <backlight_startup_executor::RegisterAddress>::address>
 2012: 9a01 ldr r2, [sp, #4]
 2014: 6002 str r2, [r0, #0]
 2016: b002 add sp, #8
 2018: bd80 pop {r7, pc}
00000942 <backlight_startup_executor::RegisterAddress>::from_register>:
 942: b580 push {r7, lr}
 944: af00 add r7, sp, #0
 946: b082 sub sp, #8
 948: b2c0 uxtb r0, r0
 94a: 9000 str r0, [sp, #0]
 94c: 9800 ldr r0, [sp, #0]
 94e: 0081 lsls r1, r0, #2
 950: a001 add r0, pc, #4 @ (adr r0, 958)
 952: 5840 ldr r0, [r0, r1]
 954: 4687 mov pc, r0
 958: 0971 .word 0x0971
 95c: 0979 .word 0x0979
 960: 0981 .word 0x0981
 964: 0989 .word 0x0989
 968: 0991 .word 0x0991
 970: a901 add r1, sp, #4
 972: 2000 movs r0, #0
 974: 7008 strb r0, [r1, #0]
 976: e00f b.n 998
 978: a901 add r1, sp, #4
 97a: 2001 movs r0, #1
 97c: 7008 strb r0, [r1, #0]
 97e: e00b b.n 998
 980: a901 add r1, sp, #4
 982: 2002 movs r0, #2
 984: 7008 strb r0, [r1, #0]
 986: e007 b.n 998
 988: a901 add r1, sp, #4
 98a: 2003 movs r0, #3
 98c: 7008 strb r0, [r1, #0]
 98e: e003 b.n 998
 990: a901 add r1, sp, #4
 992: 2004 movs r0, #4
 994: 7008 strb r0, [r1, #0]
 996: e7ff b.n 998
 998: 9801 ldr r0, [sp, #4]
 99a: b002 add sp, #8
 99c: bd80 pop {r7, pc}
000009f2 <backlight_startup_executor::RegisterAddress>::address>:
 9f2: b580 push {r7, lr}
 9f4: af00 add r7, sp, #0
 9f6: b082 sub sp, #8
 9f8: b2c0 uxtb r0, r0
 9fa: 9000 str r0, [sp, #0]
 9fc: 9800 ldr r0, [sp, #0]
 9fe: 0081 lsls r1, r0, #2
 a00: a001 add r0, pc, #4 @ (adr r0, a08)
 a02: 5840 ldr r0, [r0, r1]
 a04: 4687 mov pc, r0
 a08: 0a29 .word 0x0a29
 a0c: 0a31 .word 0x0a31
 a10: 0a39 .word 0x0a39
 a14: 0a41 .word 0x0a41
 a18: 0a49 .word 0x0a49
 a1e: 9801 ldr r0, [sp, #4]
 a20: b002 add sp, #8
 a22: bd80 pop {r7, pc}
 a28: 480f ldr r0, [pc, #60] @ (a60 <literal>)
 a2a: 9001 str r0, [sp, #4]
 a2c: e7f7 b.n a1e
 a30: 480e ldr r0, [pc, #56] @ (a64 <literal>)
 a32: 9001 str r0, [sp, #4]
 a34: e7f3 b.n a1e
 a38: 480d ldr r0, [pc, #52] @ (a68 <literal>)
 a3a: 9001 str r0, [sp, #4]
 a3c: e7ef b.n a1e
 a40: 480c ldr r0, [pc, #48] @ (a6c <literal>)
 a42: 9001 str r0, [sp, #4]
 a44: e7eb b.n a1e
 a48: 480b ldr r0, [pc, #44] @ (a70 <literal>)
 a4a: 9001 str r0, [sp, #4]
 a4c: e7e7 b.n a1e
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
 1100: b580 push {r7, lr}
 1102: af00 add r7, sp, #0
 1104: 2106 movs r1, #6
 1106: f000 f800 bl 1118 <backlight_plan::BacklightWrite]>::iter>
 110a: f000 f800 bl 1120 <core::slice::iter::Iter<backlight_plan::BacklightWrite> as core::iter::traits::iterator::Iterator>::copied::<backlight_plan::BacklightWrite>>
 110e: bd80 pop {r7, pc}
00001118 <backlight_plan::BacklightWrite]>::iter>:
 1118: bd80 pop {r7, pc}
00001120 <core::slice::iter::Iter<backlight_plan::BacklightWrite> as core::iter::traits::iterator::Iterator>::copied::<backlight_plan::BacklightWrite>>:
 1120: bd80 pop {r7, pc}
00001180 <core::iter::adapters::copied::Copied<core::slice::iter::Iter<backlight_plan::BacklightWrite>> as core::iter::traits::collect::IntoIterator>::into_iter>:
 1180: bd80 pop {r7, pc}
00001190 <core::iter::adapters::copied::Copied<core::slice::iter::Iter<backlight_plan::BacklightWrite>> as core::iter::traits::iterator::Iterator>::next>:
 1190: bd80 pop {r7, pc}
00001200 <__aeabi_memcpy4>:
 1200: 4770 bx lr
"""

CONNECTED_DISPATCH += """
00001500 <<HardwareIo as StartupIo>::crc_init>:
 1500: b580 push {r7, lr}
 1502: af00 add r7, sp, #0
 1504: f000 f800 bl 1510 <crc_startup_adapter::apply_to_hardware>
 1508: bd80 pop {r7, pc}
00001510 <crc_startup_adapter::apply_to_hardware>:
 1510: b580 push {r7, lr}
 1512: af00 add r7, sp, #0
 1514: b082 sub sp, #8
 1516: a801 add r0, sp, #4
 1518: f000 f800 bl 1600 <crc_startup_adapter::apply_with_io::<VolatileIo>>
 151c: b002 add sp, #8
 151e: bd80 pop {r7, pc}
00001600 <crc_startup_adapter::apply_with_io::<VolatileIo>>:
 1600: b580 push {r7, lr}
 1602: af00 add r7, sp, #0
 1604: b082 sub sp, #8
 1606: 9001 str r0, [sp, #4]
 1608: 2101 movs r1, #1
 160a: 020a lsls r2, r1, #8
 160c: 4904 ldr r1, [pc, #16]
 160e: f000 f800 bl 1700 <crc_startup_adapter::WordIo>::write_word>
 1612: 9801 ldr r0, [sp, #4]
 1614: 4903 ldr r1, [pc, #12]
 1616: 2200 movs r2, #0
 1618: f000 f800 bl 1700 <crc_startup_adapter::WordIo>::write_word>
 161c: b002 add sp, #8
 161e: bd80 pop {r7, pc}
 1620: 40003000 .word 0x40003000
 1624: 40003004 .word 0x40003004
00001700 <crc_startup_adapter::WordIo>::write_word>:
 1700: b580 push {r7, lr}
 1702: af00 add r7, sp, #0
 1704: 600a str r2, [r1, #0]
 1706: bd80 pop {r7, pc}
"""

# The argument-packing instructions mirror the current Cortex-M0 emission.
_witness_prefix = """
push {r7, lr}; add r7, sp, #0; sub sp, #24; add r0, sp, #16
movs r1, #0; strb r1, [r0, #0]; movs r2, #1; strb r2, [r0, #1]
strb r1, [r0, #2]; ldrb r4, [r0, #2]; add r3, sp, #20
strb r4, [r3, #2]; ldrb r4, [r0, #0]; ldrb r0, [r0, #1]
lsls r0, r0, #8; adds r0, r0, r4; strh r0, [r3, #0]
ldrh r0, [r3, #0]; ldrb r3, [r3, #2]; lsls r3, r3, #16
adds r0, r0, r3; str r2, [sp, #4]; ldr r2, [pc, #16]
str r2, [sp, #0]; ldr r2, [pc, #16]; add r3, sp, #12
"""
_hardware_prefix = """
push {r4, r5, r6, r7, lr}; add r7, sp, #12; sub sp, #28
mov r6, r0; ldr r5, [r7, #12]; ldr r4, [r7, #8]; add r0, sp, #16
strh r6, [r0, #0]; lsrs r6, r6, #16; strb r6, [r0, #2]
ldrb r6, [r0, #2]; add r0, sp, #12; strb r6, [r0, #2]
ldr r6, [sp, #16]; strh r6, [r0, #0]; ldrb r0, [r0, #2]
add r6, sp, #24; strb r0, [r6, #2]; ldr r0, [sp, #12]
strh r0, [r6, #0]; ldrh r0, [r6, #0]; ldrb r6, [r6, #2]
lsls r6, r6, #16; adds r0, r0, r6; add r6, sp, #20
str r6, [sp, #8]; str r5, [sp, #4]; str r4, [sp, #0]
"""
_adapter_prefix = """
push {r4, r5, r6, r7, lr}; add r7, sp, #12; sub sp, #52
mov r4, r3; mov r5, r2; mov r6, r0; ldr r0, [r7, #16]
str r0, [sp, #0]; ldr r2, [r7, #12]; ldr r3, [r7, #8]
add r0, sp, #16; strh r6, [r0, #0]; lsrs r6, r6, #16
strb r6, [r0, #2]; ldrb r6, [r0, #2]; add r0, sp, #12
str r0, [sp, #4]; strb r6, [r0, #2]; ldr r6, [sp, #16]
strh r6, [r0, #0]; ldr r0, [sp, #0]; ldr r6, [sp, #12]
str r0, [sp, #20]; add r0, sp, #20; str r0, [sp, #8]
strb r1, [r0, #4]; ldr r0, [sp, #4]; ldr r1, [sp, #8]
strb r6, [r1, #5]; strh r5, [r1, #6]; movs r5, #0
strb r5, [r1, #8]; str r4, [sp, #32]; str r3, [sp, #36]
str r2, [sp, #40]; movs r2, #254; strb r2, [r1, #24]
ldrb r0, [r0, #2]; add r2, sp, #48; strb r0, [r2, #2]
ldr r0, [sp, #12]; strh r0, [r2, #0]; ldrh r0, [r2, #0]
ldrb r2, [r2, #2]; lsls r2, r2, #16; adds r0, r0, r2
"""
_sequence_prefix = """
push {r7, lr}; add r7, sp, #0; sub sp, #16; str r1, [sp, #0]
mov r2, r0; ldr r0, [sp, #0]; add r1, sp, #12; strh r2, [r1, #0]
lsrs r2, r2, #16; strb r2, [r1, #2]; ldrb r1, [r1, #2]
add r2, sp, #8; str r2, [sp, #4]; strb r1, [r2, #2]
ldr r1, [sp, #12]; strh r1, [r2, #0]
"""

def _insert_prefix(disassembly, header, call_address, source, old_body=""):
    instructions = [item.strip() for item in source.replace("\n", ";").split(";") if item.strip()]
    start = call_address - 2 * len(instructions)
    old_header = f"{call_address:08x} <{header}>:\n"
    if old_body:
        old_header = f"00000300 <{header}>:\n" + old_body
    new_header = f"{start:08x} <{header}>:\n"
    for index, instruction in enumerate(instructions):
        new_header += f" {start + 2 * index:x}: 0000 {instruction}\n"
    assert old_header in disassembly
    return disassembly.replace(old_header, new_header, 1)

CONNECTED_DISPATCH = _insert_prefix(CONNECTED_DISPATCH, "witness_main", 0x200, _witness_prefix)
CONNECTED_DISPATCH = _insert_prefix(
    CONNECTED_DISPATCH, "run_hardware", 0x306, _hardware_prefix,
    " 300: b5f0 push {r4, r5, r6, r7, lr}\n"
    " 302: af03 add r7, sp, #12\n 304: b087 sub sp, #28\n",
)
CONNECTED_DISPATCH = _insert_prefix(CONNECTED_DISPATCH, "board_startup_adapters::run::<Hardware>", 0x400, _adapter_prefix)
CONNECTED_DISPATCH = _insert_prefix(CONNECTED_DISPATCH, "board_sequence::run::<Backend>", 0x500, _sequence_prefix)
CONNECTED_DISPATCH = CONNECTED_DISPATCH.replace("bl 200 <witness_main>", "bl 1cc <witness_main>", 1)
CONNECTED_DISPATCH = CONNECTED_DISPATCH.replace("bl 300 <run_hardware>", "bl 2ce <run_hardware>", 1)
CONNECTED_DISPATCH = CONNECTED_DISPATCH.replace("bl 400 <board_startup_adapters::run::<Hardware>>", "bl 3a4 <board_startup_adapters::run::<Hardware>>", 1)
CONNECTED_DISPATCH = CONNECTED_DISPATCH.replace("bl 500 <board_sequence::run::<Backend>>", "bl 4e0 <board_sequence::run::<Backend>>", 1)

for _header, _call, _prefix in (
    ("<HardwareIo as StartupIo>::device_clock_gates", 0x700, "push {r7, lr}; add r7, sp, #0"),
    ("<HardwareIo as StartupIo>::systick", 0x710, "push {r7, lr}; add r7, sp, #0; mov r0, r1"),
    ("<HardwareIo as StartupIo>::portcon", 0x720, "push {r7, lr}; add r7, sp, #0; mov r0, r1"),
    ("<HardwareIo as StartupIo>::backlight_init_hardware", 0x740, "push {r7, lr}; add r7, sp, #0"),
    ("<HardwareIo as StartupIo>::adc_init", 0x1320, "push {r7, lr}; add r7, sp, #0; mov r0, r1"),
):
    CONNECTED_DISPATCH = _insert_prefix(CONNECTED_DISPATCH, _header, _call, _prefix)

CONNECTED_DISPATCH = CONNECTED_DISPATCH.replace(
    "00000730 <<HardwareIo as StartupIo>::gpio>:\n"
    " 730: f000 f800 bl 830 <apply_gpio_startup_plan>\n"
    " 734: b00e add sp, #56\n 736: bd80 pop {r7, pc}",
    "00001900 <<HardwareIo as StartupIo>::gpio>:\n"
    " 1900: b580 push {r7, lr}\n 1902: af00 add r7, sp, #0\n"
    " 1904: b08e sub sp, #56\n 1906: a802 add r0, sp, #8\n"
    " 1908: 9001 str r0, [sp, #4]\n"
    " 190a: f000 f800 bl 1800 <gpio_init_plan::GpioPlan>::for_features>\n"
    " 190e: 9801 ldr r0, [sp, #4]\n"
    " 1910: f000 f800 bl 830 <apply_gpio_startup_plan>\n"
    " 1914: b00e add sp, #56\n 1916: bd80 pop {r7, pc}",
)
CONNECTED_DISPATCH += "\n00001800 <gpio_init_plan::GpioPlan>::for_features>:\n 1800: 4770 bx lr\n"
CONNECTED_DISPATCH = CONNECTED_DISPATCH.replace(
    "00000840 <initialize_backlight_hardware>:\n"
    " 840: f000 f800 bl 846 <backlight_startup_executor::apply_with_io>\n"
    " 844: bd80 pop {r7, pc}",
    "00001810 <initialize_backlight_hardware>:\n"
    " 1810: b580 push {r7, lr}\n 1812: af00 add r7, sp, #0\n"
    " 1814: b082 sub sp, #8\n 1816: a801 add r0, sp, #4\n"
    " 1818: f000 f800 bl 846 <backlight_startup_executor::apply_with_io>\n"
    " 181c: b002 add sp, #8\n 181e: bd80 pop {r7, pc}",
)
for _old, _new in (
    ("bl 700 <HardwareIo as StartupIo>::device_clock_gates>", "bl 6fc <HardwareIo as StartupIo>::device_clock_gates>"),
    ("bl 710 <HardwareIo as StartupIo>::systick>", "bl 70a <HardwareIo as StartupIo>::systick>"),
    ("bl 720 <HardwareIo as StartupIo>::portcon>", "bl 71a <HardwareIo as StartupIo>::portcon>"),
    ("bl 730 <HardwareIo as StartupIo>::gpio>", "bl 1900 <HardwareIo as StartupIo>::gpio>"),
    ("bl 740 <HardwareIo as StartupIo>::backlight_init_hardware>", "bl 73c <HardwareIo as StartupIo>::backlight_init_hardware>"),
    ("bl 1320 <HardwareIo as StartupIo>::adc_init>", "bl 131a <HardwareIo as StartupIo>::adc_init>"),
    ("bl 840 <initialize_backlight_hardware>", "bl 1810 <initialize_backlight_hardware>"),
    ("bl 811 <Result<(), startup_adapter_witness::board_startup_adapters::systick_startup_adapter::InvalidReload>>::map_err>", "bl 812 <Result<(), startup_adapter_witness::board_startup_adapters::systick_startup_adapter::InvalidReload>>::map_err>"),
    ("00000811 <Result<", "00000812 <Result<"),
    (" 811: bd80", " 812: bd80"),
):
    assert _old in CONNECTED_DISPATCH
    CONNECTED_DISPATCH = CONNECTED_DISPATCH.replace(_old, _new, 1)

_plan_instructions = """
push {r4, r5, r6, r7, lr}
add r7, sp, #12
sub sp, #196
movs r3, #23
lsls r3, r3, #17
orrs r1, r3
add r5, sp, #4
movs r4, #0
strb r4, [r5, #12]
str r4, [sp, #4]
str r3, [sp, #8]
str r1, [sp, #12]
add r1, sp, #20
movs r3, #1
strb r3, [r1, #12]
movs r1, #3
str r1, [sp, #20]
ldr r4, [pc, #168] @ (10cc <literal>)
str r4, [sp, #28]
movs r4, #15
lsls r5, r4, #24
bics r2, r5
add r6, sp, #36
movs r4, #2
strb r4, [r6, #12]
str r3, [sp, #36]
str r5, [sp, #40]
str r2, [sp, #44]
ldr r2, [sp, #44]
lsls r3, r3, #24
orrs r2, r3
add r5, sp, #52
strb r4, [r5, #12]
str r4, [sp, #52]
str r3, [sp, #56]
str r2, [sp, #60]
add r2, sp, #68
strb r1, [r2, #12]
str r1, [sp, #68]
ldr r2, [pc, #128] @ (10d0 <literal>)
str r2, [sp, #76]
add r3, sp, #84
movs r2, #4
strb r2, [r3, #12]
str r1, [sp, #84]
movs r1, #5
str r1, [sp, #92]
""".strip().splitlines()
for _base in range(6):
    for _offset in (12, 8, 4, 0):
        _plan_instructions.extend(
            (
                f"ldr r1, [sp, #{4 + 16 * _base + _offset}]",
                f"str r1, [sp, #{100 + 16 * _base + _offset}]",
            )
        )
_plan_instructions.extend(
    ("add r1, sp, #100", "movs r2, #96", "bl 1200 <__aeabi_memcpy4>",
     "add sp, #196", "pop {r4, r5, r6, r7, pc}")
)
_plan_body = ""
_plan_address = 0x1000
for _instruction in _plan_instructions:
    _encoded = "f000 f800" if _instruction.startswith("bl ") else "0000"
    _plan_body += f" {_plan_address:x}: {_encoded} {_instruction}\n"
    _plan_address += 4 if _instruction.startswith("bl ") else 2
_plan_body += " 10cc: 000003ff .word 0x000003ff\n 10d0: 01010000 .word 0x01010000\n"
CONNECTED_DISPATCH = re.sub(
    r"(?<=00001000 <backlight_startup_witness::backlight_startup_executor::BacklightInitPlan>::from_snapshot>:\n).*?(?=00001100 <)",
    lambda _: _plan_body,
    CONNECTED_DISPATCH,
    flags=re.S,
)
CONNECTED_WITHOUT_DISPLAY_STAGE = (
    CONNECTED_DISPATCH.replace(" 638: 1379 .word 0x1379\n", "")
    .replace(
        " 1378: 9b02 ldr r3, [sp, #8]\n"
        " 137a: 6818 ldr r0, [r3, #0]\n"
        " 137c: 68d9 ldr r1, [r3, #12]\n"
        " 137e: 691a ldr r2, [r3, #16]\n"
        " 1380: 695b ldr r3, [r3, #20]\n"
        " 1382: f000 f800 bl 1400 <HardwareIo as StartupIo>::st7565_init>\n"
        " 1386: 4601 mov r1, r0\n"
        " 1388: a805 add r0, sp, #20\n"
        " 138a: 7001 strb r1, [r0, #0]\n"
        " 138c: 7800 ldrb r0, [r0, #0]\n"
        " 138e: 28ff cmp r0, #255\n"
        " 1390: d102 bne.n 1398\n"
        " 1392: e66d b.n 670 <SequencerBackend as Backend>::call+0x70>\n"
        " 1398: 9902 ldr r1, [sp, #8]\n"
        " 139a: 9805 ldr r0, [sp, #20]\n"
        " 139c: 2201 movs r2, #1\n"
        " 139e: 720a strb r2, [r1, #8]\n"
        " 13a0: 7608 strb r0, [r1, #24]\n"
        " 13a2: e665 b.n 670 <SequencerBackend as Backend>::call+0x70>\n",
        "",
    )
)
CONNECTED_WITHOUT_DISPLAY_STAGE = re.sub(
    r"(?ms)^00001400 <<HardwareIo as StartupIo>::st7565_init>:\n.*?(?=^00001330 <)",
    "",
    CONNECTED_WITHOUT_DISPLAY_STAGE,
)

ENTRY_SELF_LOOP = CONNECTED_DISPATCH.replace(
    " 200: f000 f800 bl 2ce <run_hardware>",
    " 200: e7fe b.n 200 <witness_main>\n"
    " 202: f000 f800 bl 300 <run_hardware>",
)

WRAPPER_EARLY_RETURN = CONNECTED_DISPATCH.replace(
    " 306: f000 f800 bl 3a4 <board_startup_adapters::run::<Hardware>>",
    " 306: 4770 bx lr\n"
    " 308: f000 f800 bl 400 <board_startup_adapters::run::<Hardware>>",
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
    " 610: 9801 ldr r0, [sp, #4]",
    " 610: 4770 bx lr",
)

DISCONNECTED_BACKEND_PREFIX = CONNECTED_DISPATCH.replace(
    " 600: 7980 ldrb r0, [r0, #8]",
    " 600: 4770 bx lr\n"
    " 602: 7980 ldrb r0, [r0, #8]",
)

CONSTANT_VARIANT_RELOAD = CONNECTED_DISPATCH.replace(
    " 610: 9801 ldr r0, [sp, #4]",
    " 610: 2000 movs r0, #0",
)

OVERWRITTEN_INCOMING_VARIANT = CONNECTED_DISPATCH.replace(
    " 5fe: 9103 str r1, [sp, #12]",
    " 5fe: 9203 str r2, [sp, #12]",
)

ALWAYS_TAKEN_BACKEND_BRANCH = CONNECTED_DISPATCH.replace(
    " 610: 9801 ldr r0, [sp, #4]\n 612: 0081 lsls r1, r0, #2",
    " 610: 4280 cmp r0, r0\n 612: d01d beq.n 650",
)

UNSUPPORTED_BACKEND_CONDITIONAL = CONNECTED_DISPATCH.replace(
    " 610: 9801 ldr r0, [sp, #4]\n 612: 0081 lsls r1, r0, #2",
    " 610: 4288 cmp r0, r1\n 612: d02b beq.n 650",
)

WRONG_STOPPED_GUARD_INPUT = CONNECTED_DISPATCH.replace(
    " 600: 7980 ldrb r0, [r0, #8]",
    " 600: 7900 ldrb r0, [r0, #4]",
)

WRONG_STOPPED_GUARD_TARGET = CONNECTED_DISPATCH.replace(
    " 606: d112 bne.n 670",
    " 606: d123 bne.n 650",
)

WRONG_VARIANT_GUARD_INPUT = CONNECTED_DISPATCH.replace(
    " 60a: 9803 ldr r0, [sp, #12]",
    " 60a: 9802 ldr r0, [sp, #8]",
)

WRONG_VARIANT_GUARD_TARGET = CONNECTED_DISPATCH.replace(
    " 612: 0081 lsls r1, r0, #2",
    " 612: 0041 lsls r1, r0, #1",
)

SWAPPED_DISPATCH = CONNECTED_DISPATCH.replace(
    " 620: 1021 .word 0x1021\n 624: 1031 .word 0x1031",
    " 620: 1031 .word 0x1031\n 624: 1021 .word 0x1021",
)

UNREACHABLE_DISPATCH = CONNECTED_DISPATCH.replace(
    " 1024: f000 f800 bl 6fc <HardwareIo as StartupIo>::device_clock_gates>",
    " 1024: 4770 bx lr\n"
    " 1026: f000 f800 bl 700 <HardwareIo as StartupIo>::device_clock_gates>",
)

BRANCHED_DISPATCH = CONNECTED_DISPATCH.replace(
    " 1024: f000 f800 bl 6fc <HardwareIo as StartupIo>::device_clock_gates>",
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

ST7565_POST_CALL_SELF_LOOP = CONNECTED_DISPATCH.replace(
    " 1386: 4601 mov r1, r0",
    " 1386: e7fe b.n 1386",
)

ST7565_ERROR_PATH_BYPASS = CONNECTED_DISPATCH.replace(
    " 1390: d102 bne.n 1398",
    " 1390: d16d bne.n 670",
)

ST7565_SUCCESS_PATH_SELF_LOOP = CONNECTED_DISPATCH.replace(
    " 1392: e66d b.n 670 <SequencerBackend as Backend>::call+0x70>",
    " 1392: e7fe b.n 1392",
)

ST7565_ERROR_PATH_SELF_LOOP = CONNECTED_DISPATCH.replace(
    " 13a2: e665 b.n 670 <SequencerBackend as Backend>::call+0x70>",
    " 13a2: e7fe b.n 13a2",
)

DISCONNECTED_ADC_DISPATCH = CONNECTED_DISPATCH.replace(
    " 634: 10c1 .word 0x10c1",
    " 634: 137b .word 0x137b",
)

BYPASSED_ADC_EXECUTOR = CONNECTED_DISPATCH.replace(
    " 133a: f000 f800 bl 1348 <adc_startup_adapter::physical::apply_with_words::<Mock>>",
    " 133a: f000 f800 bl 1368 <adc_startup_adapter::noop>",
)

HARDWARE_WRAPPER_POST_CALL_SELF_LOOP = CONNECTED_DISPATCH.replace(
    " 704: bd80 pop {r7, pc}",
    " 704: e7fe b.n 704\n 706: bd80 pop {r7, pc}",
)

RUN_HARDWARE_POST_CALL_SELF_LOOP = CONNECTED_DISPATCH.replace(
    " 30a: b007 add sp, #28",
    " 30a: e7fe b.n 30a\n 30c: b007 add sp, #28",
)

RUN_HARDWARE_WRONG_RETURN = CONNECTED_DISPATCH.replace(
    " 30c: bdf0 pop {r4, r5, r6, r7, pc}",
    " 30c: bdf1 pop {r0, r4, r5, r6, r7, pc}",
)

ADC_INIT_WRONG_RETURN = CONNECTED_DISPATCH.replace(
    " 1324: bd80 pop {r7, pc}",
    " 1324: bd81 pop {r0, r7, pc}",
)

ADC_TARGET_WRONG_RETURN = CONNECTED_DISPATCH.replace(
    " 1340: bd80 pop {r7, pc}",
    " 1340: bd81 pop {r0, r7, pc}",
)

ST7565_EXECUTOR_PATH = """
00000100 <<HardwareIo as StartupIo>::st7565_init>:
 100: b580 push {r7, lr}
 102: af00 add r7, sp, #0
 104: b082 sub sp, #8
 106: f000 f800 bl 200 <target_spi0::startup_mmio>
 10a: b002 add sp, #8
 10c: bd80 pop {r7, pc}
00000200 <target_spi0::startup_mmio>:
 200: b580 push {r7, lr}
 202: af00 add r7, sp, #0
 204: b082 sub sp, #8
 206: f000 f800 bl 300 <target_spi0::startup_with_io::<Registers, Delay>>
 20a: b002 add sp, #8
 20c: bd80 pop {r7, pc}
00000300 <target_spi0::startup_with_io::<Registers, Delay>>:
 300: 4770 bx lr
"""

ST7565_INIT_ENTRY_WORD_LOOP = CONNECTED_DISPATCH.replace(
    " 1400: b580 push {r7, lr}", " 1400: e7fe e7fe .word 0xe7fee7fe"
)
ST7565_MMIO_ENTRY_WORD_LOOP = CONNECTED_DISPATCH.replace(
    " 1b00: b580 push {r7, lr}", " 1b00: e7fe e7fe .word 0xe7fee7fe"
)

ST7565_DISPATCH_CASE = """
 100: 9b02 ldr r3, [sp, #8]
 102: 6818 ldr r0, [r3, #0]
 104: 68d9 ldr r1, [r3, #12]
 106: 691a ldr r2, [r3, #16]
 108: 695b ldr r3, [r3, #20]
 10a: f000 f800 bl 200 <HardwareIo as StartupIo>::st7565_init>
 10e: 4601 mov r1, r0
 110: a805 add r0, sp, #20
 112: 7001 strb r1, [r0, #0]
 114: 7800 ldrb r0, [r0, #0]
 116: 28ff cmp r0, #255
 118: d102 bne.n 120
 11a: e011 b.n 140
 120: 9902 ldr r1, [sp, #8]
 122: 9805 ldr r0, [sp, #20]
 124: 2201 movs r2, #1
 126: 720a strb r2, [r1, #8]
 128: 7608 strb r0, [r1, #24]
 12a: e009 b.n 140
 140: b006 add sp, #24
 142: bd80 pop {r7, pc}
"""

ADAPTER_RUN_WRONG_RESULT_LOAD = CONNECTED_DISPATCH.replace(
    " 404: 980b ldr r0, [sp, #44]",
    " 404: 980a ldr r0, [sp, #40]",
)

ADAPTER_RUN_WRONG_STACK_TEARDOWN = CONNECTED_DISPATCH.replace(
    " 406: b00d add sp, #52",
    " 406: b00c add sp, #48",
)

ADAPTER_RUN_WRONG_RETURN = CONNECTED_DISPATCH.replace(
    " 408: bdf0 pop {r4, r5, r6, r7, pc}",
    " 408: bcf0 pop {r4, r5, r6, r7}",
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
    " 10b0: 9802 ldr r0, [sp, #8]",
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

BACKLIGHT_ADDRESS_CASE_SELF_LOOP = CONNECTED_DISPATCH.replace(
    " a22: bd80 pop {r7, pc}",
    " a22: e7fe b.n a22 <case_self_loop>",
)

BACKLIGHT_ADDRESS_CASE_WRONG_RETURN = CONNECTED_DISPATCH.replace(
    " a22: bd80 pop {r7, pc}",
    " a22: e001 b.n a28 <other_register_case>",
)

BACKLIGHT_ADDRESS_CASE_CROSSES_VARIANT = CONNECTED_DISPATCH.replace(
    " a2c: e7f7 b.n a1e",
    " a2c: e001 b.n a30 <other_register_case>",
)

BACKLIGHT_ADDRESS_DISPATCH_BYPASS = CONNECTED_DISPATCH.replace(
    " a04: 4687 mov pc, r0",
    " a04: 4770 bx lr",
)

BACKLIGHT_CONVERSION_DISPATCH_BYPASS = CONNECTED_DISPATCH.replace(
    " 954: 4687 mov pc, r0",
    " 954: 4770 bx lr",
)

BACKLIGHT_CONVERSION_CASE_WRONG_RETURN = CONNECTED_DISPATCH.replace(
    " 976: e00f b.n 998",
    " 976: e7ff b.n 978 <other_register_case>",
)

BACKLIGHT_WRONG_PLAN_OPERAND = CONNECTED_DISPATCH.replace(
    " 1006: 0000 movs r3, #23",
    " 1006: 0000 movs r3, #22",
)

BACKLIGHT_MISSING_FRESH_READ = CONNECTED_DISPATCH.replace(
    " 8b8: f000 f800 bl 920 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::read>",
    " 8b8: f000 f800 bl 920 <unrelated_read>",
)

BACKLIGHT_RMW_VALUE_CLOBBER = CONNECTED_DISPATCH.replace(
    " 8c0: 9804 ldr r0, [sp, #16]",
    " 8c0: 2200 movs r2, #0",
)

BACKLIGHT_WRONG_RMW_ORDER = CONNECTED_DISPATCH.replace(
    " 8c2: 430a orrs r2, r1",
    " 8c2: 438a bics r2, r1",
)

BACKLIGHT_REGISTER_READ_BYPASSES_ACCESS = CONNECTED_DISPATCH.replace(
    "00000920 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::read>:\n"
    " 920: b580 push {r7, lr}\n"
    " 922: af00 add r7, sp, #0\n"
    " 924: 4608 mov r0, r1\n"
    " 926: f000 f800 bl 942 <backlight_startup_executor::RegisterAddress>::from_register>\n"
    " 92a: f000 f800 bl 9f2 <backlight_startup_executor::RegisterAddress>::address>\n"
    " 92e: 6800 ldr r0, [r0, #0]\n"
    " 930: bd80 pop {r7, pc}",
    "00000920 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::read>:\n"
    " 920: d007 beq.n 932 <read_return>\n"
    " 922: b580 push {r7, lr}\n"
    " 924: af00 add r7, sp, #0\n"
    " 926: 4608 mov r0, r1\n"
    " 928: f000 f800 bl 942 <backlight_startup_executor::RegisterAddress>::from_register>\n"
    " 92c: f000 f800 bl 9f2 <backlight_startup_executor::RegisterAddress>::address>\n"
    " 930: 6800 ldr r0, [r0, #0]\n"
    " 932: bd80 pop {r7, pc}",
)

BACKLIGHT_REGISTER_WRITE_BYPASSES_ACCESS = CONNECTED_DISPATCH.replace(
    " 2000: b580 push {r7, lr}\n"
    " 2002: af00 add r7, sp, #0\n"
    " 2004: b082 sub sp, #8\n"
    " 2006: 9201 str r2, [sp, #4]\n"
    " 2008: 4608 mov r0, r1\n"
    " 200a: f000 f800 bl 942 <backlight_startup_executor::RegisterAddress>::from_register>\n"
    " 200e: f000 f800 bl 9f2 <backlight_startup_executor::RegisterAddress>::address>\n"
    " 2012: 9a01 ldr r2, [sp, #4]\n"
    " 2014: 6002 str r2, [r0, #0]\n"
    " 2016: b002 add sp, #8\n"
    " 2018: bd80 pop {r7, pc}",
    " 2000: d00b beq.n 201a <write_return>\n"
    " 2002: b580 push {r7, lr}\n"
    " 2004: af00 add r7, sp, #0\n"
    " 2006: b082 sub sp, #8\n"
    " 2008: 9201 str r2, [sp, #4]\n"
    " 200a: 4608 mov r0, r1\n"
    " 200c: f000 f800 bl 942 <backlight_startup_executor::RegisterAddress>::from_register>\n"
    " 2010: f000 f800 bl 9f2 <backlight_startup_executor::RegisterAddress>::address>\n"
    " 2014: 9a01 ldr r2, [sp, #4]\n"
    " 2016: 6002 str r2, [r0, #0]\n"
    " 2018: b002 add sp, #8\n"
    " 201a: bd80 pop {r7, pc}",
)

BACKLIGHT_INITIALIZER_EARLY_RETURN = """
00000840 <initialize_backlight_hardware>:
 840: 4280 cmp r0, r0
 842: d001 beq.n 848 <early_return>
 844: f000 f800 bl 846 <backlight_startup_executor::apply_with_io>
 848: bd80 pop {r7, pc}
"""

BACKLIGHT_EXECUTOR_RETURN_AT_854 = CONNECTED_DISPATCH.replace(
    " 854: 9901 ldr r1, [sp, #8]",
    " 854: 4770 bx lr",
)

BACKLIGHT_EXECUTOR_SELF_LOOP_AT_854 = CONNECTED_DISPATCH.replace(
    " 854: 9901 ldr r1, [sp, #8]",
    " 854: e7fe b.n 854 <backlight_startup_executor::apply_with_io+0xe>",
)

BACKLIGHT_EXECUTOR_EARLY_RETURN = CONNECTED_DISPATCH
_apply_header = "00000846 <backlight_startup_executor::apply_with_io>:\n"
_apply_start = BACKLIGHT_EXECUTOR_EARLY_RETURN.index(_apply_header) + len(_apply_header)
_apply_end = BACKLIGHT_EXECUTOR_EARLY_RETURN.index(
    "00000920 <<backlight_startup_executor::VolatileRegisterIo as"
)


def _shift_apply_instructions(disassembly, first_address, amount=2):
    start = disassembly.index(_apply_header) + len(_apply_header)
    end = disassembly.index(
        "00000920 <<backlight_startup_executor::VolatileRegisterIo as"
    )
    body = disassembly[start:end]

    def shift(match):
        address = int(match.group(2), 16)
        if address < first_address:
            return match.group(0)
        rest = re.sub(
            r"(\s(?:b(?:\.(?:n|w))?|b(?:eq|ne|cs|hs|cc|lo|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)(?:\.(?:n|w))?)\s+)([0-9a-fA-F]+)\b",
            lambda branch: (
                f"{branch.group(1)}"
                f"{int(branch.group(2), 16) + amount:0{len(branch.group(2))}x}"
            ),
            match.group(3),
        )
        rest = re.sub(
            r"(\.word\s+0x)([0-9a-fA-F]+)",
            lambda word: (
                f"{word.group(1)}"
                f"{int(word.group(2), 16) + amount:0{len(word.group(2))}x}"
            ),
            rest,
        )
        rest = re.sub(
            r"(\badr\s+r0,\s*)([0-9a-fA-F]+)\b",
            lambda adr: (
                f"{adr.group(1)}"
                f"{int(adr.group(2), 16) + amount:0{len(adr.group(2))}x}"
            ),
            rest,
        )
        return (
            f"{match.group(1)}"
            f"{address + amount:0{len(match.group(2))}x}:{rest}"
        )

    return disassembly[:start] + re.sub(
        r"(?m)^(\s*)([0-9a-fA-F]+):(.*)", shift, body
    ) + disassembly[end:]


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

BACKLIGHT_EXECUTOR_RETURN_AT_854 = CONNECTED_DISPATCH.replace(
    " 854: 4611 mov r1, r2",
    " 854: 4770 bx lr",
)

BACKLIGHT_EXECUTOR_SELF_LOOP_AT_854 = CONNECTED_DISPATCH.replace(
    " 854: 4611 mov r1, r2",
    " 854: e7fe b.n 854 <executor_self_loop>",
)

BACKLIGHT_EXECUTOR_BRANCH_SKIPS_PLAN = CONNECTED_DISPATCH.replace(
    " 856: f000 f800 bl 1000 <backlight_startup_executor::BacklightInitPlan>::from_snapshot>",
    " 856: e001 b.n 85c <backlight_startup_executor::apply_with_io+0x16>",
)

BACKLIGHT_EXECUTOR_CONDITIONAL_SKIPS_PLAN = CONNECTED_DISPATCH.replace(
    " 856: f000 f800 bl 1000 <backlight_startup_executor::BacklightInitPlan>::from_snapshot>",
    " 856: d000 beq.n 85a <backlight_startup_executor::apply_with_io+0x14>\n"
    " 85a: 46c0 nop",
)

_bypass_apply = CONNECTED_DISPATCH
_bypass_header = "00000846 <backlight_startup_executor::apply_with_io>:\n"
_bypass_start = _bypass_apply.index(_bypass_header) + len(_bypass_header)
_bypass_end = _bypass_apply.index(
    "00000920 <<backlight_startup_executor::VolatileRegisterIo as"
)
_bypass_body = _bypass_apply[_bypass_start:_bypass_end]
_bypass_pre = []
_bypass_post = []
for _line in _bypass_body.splitlines(keepends=True):
    _address = re.match(r"\s*([0-9a-fA-F]+):", _line)
    if _address and int(_address.group(1), 16) >= 0x856:
        _bypass_post.append(_line)
    else:
        _bypass_pre.append(_line)
_bypass_post = re.sub(
    r"(?m)^(\s*)([0-9a-fA-F]+):",
    lambda match: (
        f"{match.group(1)}"
        f"{int(match.group(2), 16) + 4:0{len(match.group(2))}x}:"
        if int(match.group(2), 16) >= 0x856
        else match.group(0)
    ),
    "".join(_bypass_post),
)
BACKLIGHT_EXECUTOR_CONDITIONAL_BYPASSES_PLAN = (
    _bypass_apply[:_bypass_start]
    + "".join(_bypass_pre)
    + " 856: 4280 cmp r0, r0\n"
    + " 858: d001 beq.n 85e\n"
    + _bypass_post
    + _bypass_apply[_bypass_end:]
)

BACKLIGHT_EXECUTOR_ALWAYS_TAKEN_EARLY_RETURN = _shift_apply_instructions(
    CONNECTED_DISPATCH, 0x846, 4
).replace(
    "00000846 <backlight_startup_executor::apply_with_io>:\n",
    "00000846 <backlight_startup_executor::apply_with_io>:\n"
    " 846: 4280 cmp r0, r0\n"
    " 848: d033 beq.n 8b2 <early_return>\n",
).replace(
    " 8b2: bd80 pop {r7, pc}",
    " 8b2: 4770 bx lr",
)

BACKLIGHT_EXECUTOR_UNSUPPORTED_BRANCH = CONNECTED_DISPATCH.replace(
    " 856: f000 f800 bl 1000 <backlight_startup_executor::BacklightInitPlan>::from_snapshot>",
    " 856: 4687 mov pc, r0",
)

BACKLIGHT_EXECUTOR_RMW_BYPASS_REJOIN = _shift_apply_instructions(
    CONNECTED_DISPATCH, 0x8bc
).replace(
    " 8b8: f000 f800 bl 920 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::read>",
    " 8b8: d001 beq.n 8be\n"
    " 8ba: f000 f800 bl 920 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::read>",
)

BACKLIGHT_PLAN_EARLY_RETURN = CONNECTED_DISPATCH.replace(
    " 1000: 0000 push {r4, r5, r6, r7, lr}",
    " 1000: 4770 bx lr",
)

BACKLIGHT_REGISTER_READ_EARLY_RETURN = CONNECTED_DISPATCH.replace(
    " 920: b580 push {r7, lr}",
    " 920: 4770 bx lr",
)

BACKLIGHT_ADDRESS_DISPATCH_BYPASS = CONNECTED_DISPATCH.replace(
    " a04: 4687 mov pc, r0",
    " a04: 4770 bx lr",
)

BACKLIGHT_ADDRESS_CASE_SELF_LOOP = CONNECTED_DISPATCH.replace(
    " a22: bd80 pop {r7, pc}",
    " a22: e7fe b.n a22 <case_self_loop>",
)

BACKLIGHT_ADDRESS_CASE_WRONG_REGISTER = CONNECTED_DISPATCH.replace(
    " a22: bd80 pop {r7, pc}",
    " a22: e000 b.n a28 <other_register_case>",
)

BACKLIGHT_CONVERSION_DISPATCH_BYPASS = CONNECTED_DISPATCH.replace(
    " 954: 4687 mov pc, r0",
    " 954: 4770 bx lr",
)

BACKLIGHT_CONVERSION_CASE_SELF_LOOP = CONNECTED_DISPATCH.replace(
    " 976: e00f b.n 998",
    " 976: e7fe b.n 976 <conversion_case_self_loop>",
)

BACKLIGHT_CONVERSION_CASE_WRONG_REGISTER = CONNECTED_DISPATCH.replace(
    " 976: e00f b.n 998",
    " 976: e7ff b.n 978 <other_register_case>",
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
    def test_elf_validator_requires_enabled_crc_configuration(self):
        vector = bytearray(0xC0)
        vector[0:4] = (0x20003FF0).to_bytes(4, "little")
        vector[4:8] = (0x101).to_bytes(4, "little")
        vector[60:64] = (0x3001).to_bytes(4, "little")
        vector_dump = "Contents of section .vector_table:\n" + "\n".join(
            f" {offset:04x} " + " ".join(
                vector[offset + i:offset + i + 4].hex() for i in range(0, 16, 4)
            )
            for offset in range(0, len(vector), 16)
        )
        sections = " 0 .vector_table 000000c0 10000000 10000000\n 1 .text 00001000 100000c0 100000c0\n"
        symbols = "\n".join(
            f"{address:08x} T {name}" for address, name in (
                (0x100, "Reset_Handler"), (0x3000, "SysTick_Handler"),
                (0x1CC, "witness_main"), (0x4E0, "board_sequence::run"),
                (0x800, "apply_device_clock_gates"), (0x810, "apply_to_hardware"),
                (0x820, "apply_portcon_startup_plan"), (0x830, "apply_gpio_startup_plan"),
                (0x1810, "initialize_backlight_hardware"), (0x1400, "StartupIo>::st7565_init"),
                (0x1B00, "target_spi0::startup_mmio"), (0x1420, "target_spi0::startup_with_io::<Mock>"),
            )
        )
        disassembly = CONNECTED_DISPATCH + "\n00003000 <SysTick_Handler>:\n 3000: 4770 bx lr\n"

        def inspect(text):
            def tool(name, *args):
                if name == "nm":
                    return symbols
                if "-h" in args:
                    return sections
                if "-s" in args:
                    return vector_dump
                return text
            with patch("check_startup_witness.run_tool", side_effect=tool):
                return validate(Path("synthetic.elf"))

        self.assertEqual(inspect(disassembly), (0x10C0, 0))
        with self.assertRaisesRegex(WitnessError, "CRC enabled configuration forwarding"):
            inspect(disassembly.replace(" 1d8: 0000 movs r2, #1", " 1d8: 0000 movs r2, #0", 1))

    def test_full_validator_rejects_disabled_or_corrupted_crc_configuration(self):
        mutations = (
            (" 1d8: 0000 movs r2, #1", " 1d8: 0000 movs r2, #0"),
            (" 1da: 0000 strb r2, [r0, #1]", " 1da: 0000 strb r1, [r0, #1]"),
            (" 1e6: 0000 ldrb r0, [r0, #1]", " 1e6: 0000 movs r0, #0"),
            (" 2d4: 0000 mov r6, r0", " 2d4: 0000 movs r6, #0"),
            (" 3ae: 0000 mov r6, r0", " 3ae: 0000 movs r6, #0"),
            (" 4e8: 0000 mov r2, r0", " 4e8: 0000 movs r2, #0"),
        )
        for original, replacement in mutations:
            with self.subTest(original=original):
                self.assertIn(original, CONNECTED_DISPATCH)
                with self.assertRaisesRegex(WitnessError, "CRC enabled configuration forwarding"):
                    validate_call_path(CONNECTED_DISPATCH.replace(original, replacement, 1))

    def test_full_validator_rejects_crc_dispatch_bypasses_and_noninstruction_targets(self):
        mutations = (
            (" 640: 13b1 .word 0x13b1", " 640: 0651 .word 0x0651"),
            (" 640: 13b1 .word 0x13b1", " 640: 13b0 .word 0x13b0"),
            (" 640: 13b1 .word 0x13b1", " 640: 13bb .word 0x13bb"),
            (" 13b4: f000 f800 bl 1500 <HardwareIo as StartupIo>::crc_init>",
             " 13b4: e7fe b.n 13b4"),
            (" 13b8: e000 b.n 670", " 13b8: e7fe b.n 13b8"),
            (" 580: 2108 movs r1, #8", " 580: 2107 movs r1, #7"),
            (" 552: 2107 movs r1, #7", " 552: 2108 movs r1, #8"),
        )
        for original, replacement in mutations:
            with self.subTest(original=original):
                self.assertIn(original, CONNECTED_DISPATCH)
                with self.assertRaises(WitnessError):
                    validate_call_path(CONNECTED_DISPATCH.replace(original, replacement, 1))

    def test_full_validator_rejects_missing_or_bypassed_crc_calls(self):
        mutations = (
            (" 1504: f000 f800 bl 1510 <crc_startup_adapter::apply_to_hardware>",
             " 1504: 4770 bx lr"),
            (" 1518: f000 f800 bl 1600 <crc_startup_adapter::apply_with_io::<VolatileIo>>",
             " 1518: e7fe b.n 1518"),
            ("00001500 <<HardwareIo as StartupIo>::crc_init>:", "00001500 <unrelated_crc>:"),
            (" 1504: f000 f800 bl 1510 <crc_startup_adapter::apply_to_hardware>",
             " 1504: f000 f800 bl 1512 <crc_startup_adapter::apply_to_hardware+0x2>"),
            (" 1518: f000 f800 bl 1600 <crc_startup_adapter::apply_with_io::<VolatileIo>>",
             " 1518: f000 f800 bl 1602 <crc_startup_adapter::apply_with_io::<VolatileIo>+0x2>"),
        )
        for original, replacement in mutations:
            with self.subTest(original=original):
                self.assertIn(original, CONNECTED_DISPATCH)
                with self.assertRaises(WitnessError):
                    validate_call_path(CONNECTED_DISPATCH.replace(original, replacement, 1))

    def test_full_validator_rejects_crc_noninstruction_entries_and_traps(self):
        mutations = (
            (" 1500: b580 push {r7, lr}", " 1500: e7fe .word 0xe7fee7fe"),
            (" 1510: b580 push {r7, lr}", " 1510: e7fe .word 0xe7fee7fe"),
            (" 1600: b580 push {r7, lr}", " 1600: e7fe .word 0xe7fee7fe"),
            (" 1502: af00 add r7, sp, #0", " 1502: de00 udf #0"),
            (" 1516: a801 add r0, sp, #4", " 1516: be00 bkpt #0"),
            (" 1612: 9801 ldr r0, [sp, #4]", " 1612: df00 svc #0"),
        )
        for original, replacement in mutations:
            with self.subTest(original=original):
                self.assertIn(original, CONNECTED_DISPATCH)
                with self.assertRaises(WitnessError):
                    validate_call_path(CONNECTED_DISPATCH.replace(original, replacement, 1))

    def test_full_validator_rejects_crc_frame_restore_and_write_mutations(self):
        mutations = (
            (" 1500: b580 push {r7, lr}", " 1500: b581 push {r0, r7, lr}"),
            (" 1508: bd80 pop {r7, pc}", " 1508: bd81 pop {r0, r7, pc}"),
            (" 1514: b082 sub sp, #8", " 1514: b083 sub sp, #12"),
            (" 151c: b002 add sp, #8", " 151c: b001 add sp, #4"),
            (" 151e: bd80 pop {r7, pc}", " 151e: 4770 bx lr"),
            (" 1604: b082 sub sp, #8", " 1604: b083 sub sp, #12"),
            (" 161c: b002 add sp, #8", " 161c: b001 add sp, #4"),
            (" 161e: bd80 pop {r7, pc}", " 161e: e7fe b.n 161e"),
            (" 1620: 40003000 .word 0x40003000", " 1620: 40003008 .word 0x40003008"),
            (" 1616: 2200 movs r2, #0", " 1616: 2201 movs r2, #1"),
            (" 1704: 600a str r2, [r1, #0]", " 1704: 700a strb r2, [r1, #0]"),
        )
        for original, replacement in mutations:
            with self.subTest(original=original):
                self.assertIn(original, CONNECTED_DISPATCH)
                with self.assertRaises(WitnessError):
                    validate_call_path(CONNECTED_DISPATCH.replace(original, replacement, 1))

    def test_full_validator_rejects_extra_backend_call_source(self):
        for target in ("5f6", "1020"):
            with self.subTest(target=target):
                extra = CONNECTED_DISPATCH + (
                    "\n00003000 <unexpected_caller>:\n"
                    f" 3000: f000 f800 bl {target} <alias>\n"
                    " 3004: 4770 bx lr\n"
                )
                with self.assertRaisesRegex(WitnessError, "unexpected call sources"):
                    validate_call_path(extra)

    def test_full_validator_rejects_corrupted_display_argument_forwarding(self):
        for original, replacement in (
            (" 1408: 4610 mov r0, r2", " 1408: 2000 movs r0, #0"),
            (" 1410: 9901 ldr r1, [sp, #4]", " 1410: 9900 ldr r1, [sp, #0]"),
            (" 1b06: 4613 mov r3, r2", " 1b06: 2300 movs r3, #0"),
            (" 1b0c: a801 add r0, sp, #4", " 1b0c: a800 add r0, sp, #0"),
        ):
            with self.subTest(original=original):
                self.assertIn(original, CONNECTED_DISPATCH)
                with self.assertRaisesRegex(WitnessError, "concrete wrapper.*continuation"):
                    validate_call_path(CONNECTED_DISPATCH.replace(original, replacement, 1))

    def test_accepts_reachable_st7565_dispatch_variant(self):
        validate_st7565_backend_case(ST7565_DISPATCH_CASE, 0x100)

    def test_rejects_st7565_dispatch_call_after_early_return(self):
        disconnected = ST7565_DISPATCH_CASE.replace(
            " 10a: f000 f800 bl 200 <HardwareIo as StartupIo>::st7565_init>",
            " 10a: 4770 bx lr\n"
            " 10e: f000 f800 bl 200 <HardwareIo as StartupIo>::st7565_init>",
        )
        with self.assertRaisesRegex(
            WitnessError, "display variant has no reachable ST7565 startup call"
        ):
            validate_st7565_backend_case(disconnected, 0x100)

    def test_rejects_bypassed_st7565_dispatch_call_with_symbol_retained(self):
        bypassed = ST7565_DISPATCH_CASE.replace(
            " 10a: f000 f800 bl 200 <HardwareIo as StartupIo>::st7565_init>",
            " 10a: f000 f800 bl 300 <startup_noop>",
        ) + "\n 14a: f000 f800 bl 200 <HardwareIo as StartupIo>::st7565_init>\n"
        with self.assertRaisesRegex(WitnessError, "display variant has no reachable ST7565 startup call"):
            validate_st7565_backend_case(bypassed, 0x100)

    def test_rejects_st7565_post_call_self_loop(self):
        self_loop = ST7565_DISPATCH_CASE.replace(
            " 10e: 4601 mov r1, r0",
            " 10e: e7fe b.n 10e",
        )
        with self.assertRaisesRegex(
            WitnessError, "display variant has an unexpected argument/result continuation"
        ):
            validate_st7565_backend_case(self_loop, 0x100)

    def test_rejects_jump_table_word_before_later_st7565_call(self):
        disconnected = (
            " 100: e7fe .word 0xe7fee7fe\n"
            " 104: f000 f800 bl 200 <HardwareIo as StartupIo>::st7565_init>\n"
        )
        with self.assertRaisesRegex(
            WitnessError, "display variant has an unexpected argument/result continuation"
        ):
            validate_st7565_backend_case(disconnected, 0x100)

    def test_accepts_concrete_st7565_executor_call_chain(self):
        validate_st7565_executor_path(ST7565_EXECUTOR_PATH)

    def test_rejects_trapping_st7565_executor_wrapper_entries(self):
        for original in (
            " 100: b580 push {r7, lr}",
            " 200: b580 push {r7, lr}",
        ):
            with self.subTest(wrapper=original):
                trapped = ST7565_EXECUTOR_PATH.replace(
                    original, original.split(":")[0] + ": de00 udf #0"
                )
                with self.assertRaisesRegex(WitnessError, "unsupported trap instruction"):
                    validate_st7565_executor_path(trapped)

    def test_rejects_unbalanced_st7565_executor_wrapper_restores(self):
        for original in (
            " 10c: bd80 pop {r7, pc}",
            " 20c: bd80 pop {r7, pc}",
        ):
            with self.subTest(wrapper=original):
                unbalanced = ST7565_EXECUTOR_PATH.replace(
                    original,
                    original.replace("bd80 pop {r7, pc}", "bc81 pop {r0, r7, pc}"),
                )
                with self.assertRaisesRegex(
                    WitnessError, "non-returning call continuation"
                ):
                    validate_st7565_executor_path(unbalanced)

    def test_rejects_unbalanced_st7565_executor_wrapper_entries(self):
        for original in (
            (" 100: b580 push {r7, lr}", " 100: b581 push {r0, r7, lr}"),
            (" 200: b580 push {r7, lr}", " 200: b581 push {r0, r7, lr}"),
            (" 104: b082 sub sp, #8", " 104: b083 sub sp, #12"),
            (" 204: b082 sub sp, #8", " 204: b083 sub sp, #12"),
        ):
            with self.subTest(entry=original):
                unbalanced = ST7565_EXECUTOR_PATH.replace(*original)
                with self.assertRaisesRegex(WitnessError, "unexpected frame setup"):
                    validate_st7565_executor_path(unbalanced)

    def test_rejects_extra_st7565_executor_wrapper_stack_allocation(self):
        cases = (
            (
                (" 106: f000 f800 bl 200 <target_spi0::startup_mmio>",
                 " 106: b081 sub sp, #4\n 108: f000 f800 bl 200 <target_spi0::startup_mmio>"),
                (" 10a: b002 add sp, #8", " 10c: b002 add sp, #8"),
                (" 10c: bd80 pop {r7, pc}", " 10e: bd80 pop {r7, pc}"),
            ),
            (
                (" 206: f000 f800 bl 300 <target_spi0::startup_with_io::<Registers, Delay>>",
                 " 206: b081 sub sp, #4\n"
                 " 208: f000 f800 bl 300 <target_spi0::startup_with_io::<Registers, Delay>>"),
                (" 20a: b002 add sp, #8", " 20c: b002 add sp, #8"),
                (" 20c: bd80 pop {r7, pc}", " 20e: bd80 pop {r7, pc}"),
            ),
        )
        for replacements in cases:
            with self.subTest(call=replacements[0][0]):
                unbalanced = ST7565_EXECUTOR_PATH
                for original, replacement in replacements:
                    unbalanced = unbalanced.replace(original, replacement)
                with self.assertRaisesRegex(WitnessError, "unexpected frame setup"):
                    validate_st7565_executor_path(unbalanced)

    def test_rejects_st7565_call_disconnected_after_early_return(self):
        disconnected = ST7565_EXECUTOR_PATH.replace(
            " 106: f000 f800 bl 200 <target_spi0::startup_mmio>",
            " 106: 4770 bx lr\n"
            " 108: f000 f800 bl 200 <target_spi0::startup_mmio>",
        )
        with self.assertRaisesRegex(WitnessError, "target ST7565 init path does not reach"):
            validate_st7565_executor_path(disconnected)

    def test_rejects_st7565_call_bypassed_to_unrelated_target(self):
        bypassed = ST7565_EXECUTOR_PATH.replace(
            " 106: f000 f800 bl 200 <target_spi0::startup_mmio>",
            " 106: f000 f800 bl 400 <startup_noop>",
        ) + "\n00000400 <startup_noop>:\n 400: 4770 bx lr\n"
        with self.assertRaisesRegex(WitnessError, "target ST7565 init path does not reach"):
            validate_st7565_executor_path(bypassed)

    def test_rejects_hardware_display_wrapper_entry_word_loop(self):
        with self.assertRaisesRegex(WitnessError, "no instruction at its symbol entry"):
            validate_call_path(ST7565_INIT_ENTRY_WORD_LOOP)

    def test_rejects_mmio_display_wrapper_entry_word_loop(self):
        with self.assertRaisesRegex(WitnessError, "no instruction at its symbol entry"):
            validate_call_path(ST7565_MMIO_ENTRY_WORD_LOOP)

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
        self.assertIn("618: 4687 mov pc, r0", DISCONNECTED_BACKEND_PREFIX)
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

    def test_rejects_disconnected_adc_backend_dispatch(self):
        with self.assertRaisesRegex(
            WitnessError, "variant 5.*unexpected continuation"
        ):
            validate_call_path(DISCONNECTED_ADC_DISPATCH)

    def test_rejects_bypassed_adc_executor(self):
        with self.assertRaisesRegex(
            WitnessError, "target ADC executor.*does not reach"
        ):
            validate_call_path(BYPASSED_ADC_EXECUTOR)

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

    def test_rejects_extra_register_restore_in_run_hardware(self):
        with self.assertRaisesRegex(
            WitnessError, "run_hardware.*non-returning call continuation"
        ):
            validate_call_path(RUN_HARDWARE_WRONG_RETURN)

    def test_rejects_extra_register_restore_in_adc_init(self):
        with self.assertRaisesRegex(
            WitnessError, "target adc_init.*non-returning call continuation"
        ):
            validate_call_path(ADC_INIT_WRONG_RETURN)

    def test_rejects_extra_register_restore_in_adc_target_executor(self):
        with self.assertRaisesRegex(
            WitnessError, "target ADC executor.*non-returning call continuation"
        ):
            validate_call_path(ADC_TARGET_WRONG_RETURN)

    def test_rejects_adapter_result_load_mismatch(self):
        with self.assertRaisesRegex(
            WitnessError, "board startup adapter result path has an unexpected continuation"
        ):
            validate_call_path(ADAPTER_RUN_WRONG_RESULT_LOAD)

    def test_rejects_adapter_stack_teardown_mismatch(self):
        with self.assertRaisesRegex(
            WitnessError, "board startup adapter result path has an unexpected continuation"
        ):
            validate_call_path(ADAPTER_RUN_WRONG_STACK_TEARDOWN)

    def test_rejects_adapter_return_mismatch(self):
        with self.assertRaisesRegex(
            WitnessError, "board startup adapter result path has an unexpected continuation"
        ):
            validate_call_path(ADAPTER_RUN_WRONG_RETURN)

    def test_rejects_full_validator_display_post_call_self_loop(self):
        with self.assertRaisesRegex(
            WitnessError, "display variant has an unexpected argument/result continuation"
        ):
            validate_call_path(ST7565_POST_CALL_SELF_LOOP)

    def test_rejects_full_validator_display_error_path_bypass(self):
        with self.assertRaisesRegex(
            WitnessError, "display variant has an unexpected argument/result continuation"
        ):
            validate_call_path(ST7565_ERROR_PATH_BYPASS)

    def test_rejects_full_validator_display_success_path_self_loop(self):
        with self.assertRaisesRegex(
            WitnessError, "display variant result paths do not reach the shared return epilogue"
        ):
            validate_call_path(ST7565_SUCCESS_PATH_SELF_LOOP)

    def test_rejects_full_validator_display_error_path_self_loop(self):
        with self.assertRaisesRegex(
            WitnessError, "display variant result paths do not reach the shared return epilogue"
        ):
            validate_call_path(ST7565_ERROR_PATH_SELF_LOOP)

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

    def test_rejects_full_validator_fixture_without_display_stage(self):
        with self.assertRaisesRegex(
            WitnessError, "disassembly is missing StartupIo>::st7565_init"
        ):
            validate_call_path(CONNECTED_WITHOUT_DISPLAY_STAGE)

    def test_full_validator_rejects_trapping_st7565_executor_wrappers(self):
        for original in (
            " 1400: b580 push {r7, lr}",
            " 1b00: b580 push {r7, lr}",
        ):
            with self.subTest(wrapper=original):
                trapped = CONNECTED_DISPATCH.replace(
                    original, original.split(":")[0] + ": de00 udf #0"
                )
                with self.assertRaisesRegex(
                    WitnessError, "unsupported trap instruction"
                ):
                    validate_call_path(trapped)

    def test_full_validator_rejects_unbalanced_st7565_executor_restores(self):
        for original in (
            " 1418: bd80 pop {r7, pc}",
            " 1b14: bd80 pop {r7, pc}",
        ):
            with self.subTest(wrapper=original):
                unbalanced = CONNECTED_DISPATCH.replace(
                    original,
                    original.replace("bd80 pop {r7, pc}", "bc81 pop {r0, r7, pc}"),
                )
                with self.assertRaisesRegex(
                    WitnessError, "non-returning call continuation"
                ):
                    validate_call_path(unbalanced)

    def test_full_validator_rejects_unbalanced_st7565_executor_entries(self):
        for original in (
            (" 1400: b580 push {r7, lr}", " 1400: b581 push {r0, r7, lr}"),
            (" 1b00: b580 push {r7, lr}", " 1b00: b581 push {r0, r7, lr}"),
            (" 1404: b082 sub sp, #8", " 1404: b083 sub sp, #12"),
            (" 1b04: b082 sub sp, #8", " 1b04: b083 sub sp, #12"),
        ):
            with self.subTest(entry=original):
                unbalanced = CONNECTED_DISPATCH.replace(*original)
                with self.assertRaisesRegex(WitnessError, "unexpected frame setup"):
                    validate_call_path(unbalanced)

    def test_full_validator_rejects_extra_st7565_executor_stack_allocation(self):
        cases = (
            (
                (" 1412: f000 f800 bl 1b00 <target_spi0::startup_mmio>",
                 " 1412: b081 sub sp, #4\n 1414: f000 f800 bl 1b00 <target_spi0::startup_mmio>"),
                (" 1416: b002 add sp, #8", " 1418: b002 add sp, #8"),
                (" 1418: bd80 pop {r7, pc}", " 141a: bd80 pop {r7, pc}"),
            ),
            (
                (" 1b0e: f000 f800 bl 1420 <target_spi0::startup_with_io::<Registers, Delay>>",
                 " 1b0e: b081 sub sp, #4\n"
                 " 1b10: f000 f800 bl 1420 <target_spi0::startup_with_io::<Registers, Delay>>"),
                (" 1b12: b002 add sp, #8", " 1b14: b002 add sp, #8"),
                (" 1b14: bd80 pop {r7, pc}", " 1b16: bd80 pop {r7, pc}"),
            ),
        )
        for replacements in cases:
            with self.subTest(call=replacements[0][0]):
                unbalanced = CONNECTED_DISPATCH
                for original, replacement in replacements:
                    unbalanced = unbalanced.replace(original, replacement)
                with self.assertRaisesRegex(WitnessError, "unexpected frame setup"):
                    validate_call_path(unbalanced)

    def test_rejects_required_calls_that_do_not_target_the_symbol_entry(self):
        mutations = (
            (
                " 100: f000 f800 bl 1cc <witness_main>",
                " 100: f000 f800 bl 1ce <witness_main+0x2>",
            ),
            (
                " 740: f000 f800 bl 1810 <initialize_backlight_hardware>",
                " 740: f000 f800 bl 1812 <initialize_backlight_hardware+0x2>",
            ),
            (
                " 1818: f000 f800 bl 846 <backlight_startup_executor::apply_with_io>",
                " 1818: f000 f800 bl 8ac <backlight_startup_executor::apply_with_io>",
            ),
            (
                " 1818: f000 f800 bl 846 <backlight_startup_executor::apply_with_io>",
                " 1818: f000 f800 bl 846 <other::backlight_startup_executor::apply_with_io>",
            ),
            (
                " 856: f000 f800 bl 1000 <backlight_startup_executor::BacklightInitPlan>::from_snapshot>",
                " 856: f000 f800 bl 1002 <backlight_startup_executor::BacklightInitPlan>::from_snapshot+0x2>",
            ),
            (
                " 926: f000 f800 bl 942 <backlight_startup_executor::RegisterAddress>::from_register>",
                " 926: f000 f800 bl 944 <backlight_startup_executor::RegisterAddress>::from_register+0x2>",
            ),
            (
                " 8b8: f000 f800 bl 920 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::read>",
                " 8b8: f000 f800 bl 922 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::read+0x2>",
            ),
            (
                " 8c6: f000 f800 bl 2000 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::write>",
                " 8c6: f000 f800 bl 2002 <<backlight_startup_executor::VolatileRegisterIo as backlight_startup_executor::RegisterIo>::write+0x2>",
            ),
        )
        for original, replacement in mutations:
            with self.subTest(replacement=replacement):
                self.assertIn(original, CONNECTED_DISPATCH)
                mutated = CONNECTED_DISPATCH.replace(original, replacement, 1)
                with self.assertRaisesRegex(
                    WitnessError, "required call target.*symbol entry"
                ):
                    validate_call_path(mutated)

    def test_rejects_call_to_different_entry_with_similar_symbol_name(self):
        original = (
            " 1818: f000 f800 bl 846 <backlight_startup_executor::apply_with_io>"
        )
        generic_call = (
            " 1818: f000 f800 bl 846 "
            "<backlight_startup_executor::apply_with_io::<VolatileRegisterIo>>"
        )
        generic_header = (
            "00000846 <backlight_startup_executor::apply_with_io::<VolatileRegisterIo>>:"
        )
        self.assertIn(original, CONNECTED_DISPATCH)
        mutated = CONNECTED_DISPATCH.replace(original, generic_call, 1).replace(
            "00000846 <backlight_startup_executor::apply_with_io>:",
            generic_header,
            1,
        )
        validate_call_path(mutated)
        replacement = (
            " 1818: f000 f800 bl 3000 "
            "<other::backlight_startup_executor::apply_with_io::<VolatileRegisterIo>>"
        )
        mutated = mutated.replace(generic_call, replacement, 1)
        mutated += (
            "\n00003000 "
            "<other::backlight_startup_executor::apply_with_io::<VolatileRegisterIo>>:\n"
            " 3000: bd80 pop {r7, pc}\n"
        )
        self.assertIn(
            "00003000 <other::backlight_startup_executor::apply_with_io::<VolatileRegisterIo>>",
            mutated,
        )
        with self.assertRaisesRegex(WitnessError, "required call target.*symbol entry"):
            validate_call_path(mutated)

    def test_rejects_initializer_call_redirected_to_lookalike_noop(self):
        original = " 740: f000 f800 bl 1810 <initialize_backlight_hardware>"
        replacement = " 740: f000 f800 bl 3000 <initialize_backlight_hardware_noop>"
        self.assertIn(original, CONNECTED_DISPATCH)
        mutated = CONNECTED_DISPATCH.replace(original, replacement, 1)
        mutated += (
            "\n00003000 <initialize_backlight_hardware_noop>:\n"
            " 3000: 4770 bx lr\n"
        )
        self.assertIn("00003000 <initialize_backlight_hardware_noop>", mutated)
        with self.assertRaisesRegex(
            WitnessError, "target backlight_init_hardware path does not reach"
        ):
            validate_call_path(mutated)

    def test_rejects_unmatched_backend_call_target(self):
        original = (
            " 656: f000 f8f3 bl 1300 <<startup_adapter_witness::"
            "board_startup_adapters::HardwareIo as "
            "startup_adapter_witness::board_startup_adapters::StartupIo>::placeholder>"
        )
        replacement = (
            " 656: f000 f8f3 bl 3000 <<OtherIo as "
            "startup_adapter_witness::board_startup_adapters::StartupIo>::placeholder>"
        )
        self.assertIn(original, CONNECTED_DISPATCH)
        mutated = CONNECTED_DISPATCH.replace(original, replacement, 1)
        mutated += (
            "\n00003000 <<OtherIo as "
            "startup_adapter_witness::board_startup_adapters::StartupIo>::placeholder>:\n"
            " 3000: 4770 bx lr\n"
        )
        with self.assertRaisesRegex(
            WitnessError, "required caller board_sequence::Backend>::call.*unexpected call target"
        ):
            validate_call_path(mutated)

    def test_rejects_allowed_helper_lookalike_symbols(self):
        mutations = (
            (
                " 656: f000 f8f3 bl 1300 <<startup_adapter_witness::"
                "board_startup_adapters::HardwareIo as "
                "startup_adapter_witness::board_startup_adapters::StartupIo>::placeholder>",
                " 656: f000 f8f3 bl 3000 <<OtherHardwareIo as "
                "startup_adapter_witness::board_startup_adapters::StartupIo>::placeholder>",
                "\n00003000 <<OtherHardwareIo as "
                "startup_adapter_witness::board_startup_adapters::StartupIo>::placeholder>:\n"
                " 3000: 4770 bx lr\n",
            ),
            (
                " 1040: f000 f800 bl 1310 <<core::result::Result<(), "
                "startup_adapter_witness::board_startup_adapters::StartupError>>::is_err>",
                " 1040: f000 f800 bl 3020 <<Other::Result<(), "
                "startup_adapter_witness::board_startup_adapters::StartupError>>::is_err>",
                "\n00003020 <<Other::Result<(), "
                "startup_adapter_witness::board_startup_adapters::StartupError>>::is_err>:\n"
                " 3020: bd80 pop {r7, pc}\n",
            ),
        )
        for original, replacement, appended in mutations:
            with self.subTest(replacement=replacement):
                self.assertIn(original, CONNECTED_DISPATCH)
                mutated = CONNECTED_DISPATCH.replace(original, replacement, 1) + appended
                with self.assertRaisesRegex(
                    WitnessError, "required caller board_sequence::Backend>::call.*unexpected call target"
                ):
                    validate_call_path(mutated)

    def test_rejects_copied_iterator_call_to_interior_entry(self):
        original = (
            " 110a: f000 f800 bl 1120 "
            "<core::slice::iter::Iter<backlight_plan::BacklightWrite> as "
            "core::iter::traits::iterator::Iterator>::copied::<backlight_plan::BacklightWrite>>"
        )
        replacement = (
            " 110a: f000 f800 bl 1122 "
            "<core::slice::iter::Iter<backlight_plan::BacklightWrite> as "
            "core::iter::traits::iterator::Iterator>::copied::<backlight_plan::BacklightWrite>+0x2>"
        )
        self.assertIn(original, CONNECTED_DISPATCH)
        mutated = CONNECTED_DISPATCH.replace(original, replacement, 1)
        with self.assertRaisesRegex(WitnessError, "required call target.*symbol entry"):
            validate_call_path(mutated)

    def test_rejects_backlight_executor_evidence_after_address_preserving_early_return(self):
        self.assertIn("00000846 <backlight_startup_executor::apply_with_io>", BACKLIGHT_EXECUTOR_EARLY_RETURN)
        self.assertIn(" 846: 4770 bx lr", BACKLIGHT_EXECUTOR_EARLY_RETURN)
        self.assertIn(" 858: f000 f800 bl 1000", BACKLIGHT_EXECUTOR_EARLY_RETURN)

        with self.assertRaisesRegex(WitnessError, "backlight executor.*initialization plan"):
            validate_call_path(BACKLIGHT_EXECUTOR_EARLY_RETURN)

    def test_rejects_executor_return_at_854_with_register_evidence_retained(self):
        self.assertIn(" 854: 4770 bx lr", BACKLIGHT_EXECUTOR_RETURN_AT_854)
        self.assertIn(" 856: f000 f800 bl 1000", BACKLIGHT_EXECUTOR_RETURN_AT_854)

        with self.assertRaisesRegex(WitnessError, "backlight executor"):
            validate_call_path(BACKLIGHT_EXECUTOR_RETURN_AT_854)

    def test_rejects_executor_self_loop_at_854_with_register_evidence_retained(self):
        self.assertIn(" 854: e7fe b.n 854", BACKLIGHT_EXECUTOR_SELF_LOOP_AT_854)
        self.assertIn(" 856: f000 f800 bl 1000", BACKLIGHT_EXECUTOR_SELF_LOOP_AT_854)

        with self.assertRaisesRegex(WitnessError, "backlight executor"):
            validate_call_path(BACKLIGHT_EXECUTOR_SELF_LOOP_AT_854)

    def test_rejects_backlight_executor_branch_that_skips_plan_creation(self):
        with self.assertRaisesRegex(WitnessError, "backlight executor.*initialization plan"):
            validate_call_path(BACKLIGHT_EXECUTOR_BRANCH_SKIPS_PLAN)

    def test_rejects_backlight_executor_conditional_branch_that_skips_plan_creation(self):
        with self.assertRaisesRegex(WitnessError, "backlight executor.*initialization plan"):
            validate_call_path(BACKLIGHT_EXECUTOR_CONDITIONAL_SKIPS_PLAN)

    def test_rejects_always_taken_branch_over_plan_call_with_call_retained(self):
        self.assertIn("cmp r0, r0", BACKLIGHT_EXECUTOR_CONDITIONAL_BYPASSES_PLAN)
        self.assertIn("beq.n 85e", BACKLIGHT_EXECUTOR_CONDITIONAL_BYPASSES_PLAN)
        self.assertIn("bl 1000 <backlight_startup_executor::BacklightInitPlan>::from_snapshot>",
                      BACKLIGHT_EXECUTOR_CONDITIONAL_BYPASSES_PLAN)

    def test_rejects_always_taken_branch_to_early_return_with_retained_plan(self):
        self.assertIn(" 846: 4280 cmp r0, r0", BACKLIGHT_EXECUTOR_ALWAYS_TAKEN_EARLY_RETURN)
        self.assertIn(" 848: d033 beq.n 8b2", BACKLIGHT_EXECUTOR_ALWAYS_TAKEN_EARLY_RETURN)
        self.assertIn(
            " 85a: f000 f800 bl 1000 <backlight_startup_executor::BacklightInitPlan>::from_snapshot>",
            BACKLIGHT_EXECUTOR_ALWAYS_TAKEN_EARLY_RETURN,
        )
        self.assertIn(" 8b2: 4770 bx lr", BACKLIGHT_EXECUTOR_ALWAYS_TAKEN_EARLY_RETURN)
        with self.assertRaisesRegex(
            WitnessError, "backlight executor"
        ):
            validate_call_path(BACKLIGHT_EXECUTOR_ALWAYS_TAKEN_EARLY_RETURN)

    def test_rejects_backlight_rmw_branch_that_bypasses_read_and_rejoins(self):
        self.assertIn("beq.n 8be", BACKLIGHT_EXECUTOR_RMW_BYPASS_REJOIN)
        self.assertIn("8ba: f000 f800 bl 920", BACKLIGHT_EXECUTOR_RMW_BYPASS_REJOIN)
        with self.assertRaisesRegex(
            WitnessError, "backlight executor write case 0 has an unsupported branch"
        ):
            validate_call_path(BACKLIGHT_EXECUTOR_RMW_BYPASS_REJOIN)

    def test_rejects_always_taken_branch_over_plan_call_with_call_retained(self):
        self.assertIn("cmp r0, r0", BACKLIGHT_EXECUTOR_CONDITIONAL_BYPASSES_PLAN)
        self.assertIn("beq.n 85e", BACKLIGHT_EXECUTOR_CONDITIONAL_BYPASSES_PLAN)
        with self.assertRaisesRegex(WitnessError, "backlight executor"):
            validate_call_path(BACKLIGHT_EXECUTOR_CONDITIONAL_BYPASSES_PLAN)

    def test_rejects_unsupported_backlight_executor_indirect_branch(self):
        with self.assertRaisesRegex(WitnessError, "backlight executor.*unsupported indirect branch"):
            validate_call_path(BACKLIGHT_EXECUTOR_UNSUPPORTED_BRANCH)

    def test_rejects_backlight_plan_evidence_after_early_return(self):
        with self.assertRaisesRegex(WitnessError, "backlight plan operands"):
            validate_call_path(BACKLIGHT_PLAN_EARLY_RETURN)

    def test_rejects_plan_construction_bypassed_by_conditional_return(self):
        start = CONNECTED_DISPATCH.index(" 1000: 0000 push")
        end = CONNECTED_DISPATCH.index("00001100 <", start)
        body = CONNECTED_DISPATCH[start:end]
        shifted = re.sub(
            r"(?m)^(\s*)([0-9a-f]+):",
            lambda match: f"{match.group(1)}{int(match.group(2), 16) + 4:x}:",
            body,
        )
        shifted = re.sub(
            r"\((10cc|10d0) <literal>\)",
            lambda match: f"({int(match.group(1), 16) + 4:x} <literal>)",
            shifted,
        )
        mutated = (
            CONNECTED_DISPATCH[:start]
            + " 1000: 4280 cmp r0, r0\n 1002: d063 beq.n 10cc <plan_return>\n"
            + shifted
            + CONNECTED_DISPATCH[end:]
        )
        self.assertIn("1002: d063 beq.n 10cc", mutated)
        self.assertIn("10cc: 0000 pop {r4, r5, r6, r7, pc}", mutated)
        with self.assertRaisesRegex(WitnessError, "backlight plan"):
            validate_call_path(mutated)

    def test_rejects_register_mapping_evidence_after_early_return(self):
        with self.assertRaisesRegex(
            WitnessError, "backlight register read.*instruction continuation"
        ):
            validate_call_path(BACKLIGHT_REGISTER_READ_EARLY_RETURN)

    def test_rejects_bypassed_address_mapping_dispatch(self):
        with self.assertRaisesRegex(
            WitnessError, "backlight register address mapping.*indexed dispatch"
        ):
            validate_call_path(BACKLIGHT_ADDRESS_DISPATCH_BYPASS)

    def test_rejects_address_case_self_loop(self):
        with self.assertRaisesRegex(
            WitnessError, "backlight register address mapping.*shared return epilogue"
        ):
            validate_call_path(BACKLIGHT_ADDRESS_CASE_SELF_LOOP)

    def test_rejects_address_case_returning_another_register(self):
        with self.assertRaisesRegex(
            WitnessError, "backlight register address mapping.*shared return epilogue"
        ):
            validate_call_path(BACKLIGHT_ADDRESS_CASE_WRONG_REGISTER)

    def test_rejects_bypassed_register_conversion_dispatch(self):
        with self.assertRaisesRegex(
            WitnessError, "backlight register conversion.*indexed dispatch"
        ):
            validate_call_path(BACKLIGHT_CONVERSION_DISPATCH_BYPASS)

    def test_rejects_register_conversion_case_self_loop(self):
        with self.assertRaisesRegex(
            WitnessError, "backlight register conversion cases do not share their return epilogue"
        ):
            validate_call_path(BACKLIGHT_CONVERSION_CASE_SELF_LOOP)

    def test_rejects_register_conversion_case_returning_another_register(self):
        with self.assertRaisesRegex(
            WitnessError, "backlight register conversion cases do not share their return epilogue"
        ):
            validate_call_path(BACKLIGHT_CONVERSION_CASE_WRONG_REGISTER)

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

    def test_rejects_address_case_self_loop_instead_of_shared_epilogue(self):
        self.assertIn("a22: e7fe b.n a22", BACKLIGHT_ADDRESS_CASE_SELF_LOOP)
        self.assertIn("a28: 480f ldr r0", BACKLIGHT_ADDRESS_CASE_SELF_LOOP)

        with self.assertRaisesRegex(WitnessError, "address.*shared return epilogue"):
            validate_call_path(BACKLIGHT_ADDRESS_CASE_SELF_LOOP)

    def test_rejects_address_case_branch_to_another_register_case(self):
        self.assertIn("a22: e001 b.n a28", BACKLIGHT_ADDRESS_CASE_WRONG_RETURN)
        self.assertIn("a28: 480f ldr r0", BACKLIGHT_ADDRESS_CASE_WRONG_RETURN)

        with self.assertRaisesRegex(WitnessError, "address.*shared return epilogue"):
            validate_call_path(BACKLIGHT_ADDRESS_CASE_WRONG_RETURN)

    def test_rejects_address_case_that_returns_another_variant_value(self):
        self.assertIn("a2c: e001 b.n a30", BACKLIGHT_ADDRESS_CASE_CROSSES_VARIANT)
        self.assertIn("a30: 480e ldr r0", BACKLIGHT_ADDRESS_CASE_CROSSES_VARIANT)

        with self.assertRaisesRegex(WitnessError, "address case.*shared return epilogue"):
            validate_call_path(BACKLIGHT_ADDRESS_CASE_CROSSES_VARIANT)

    def test_rejects_address_dispatch_replaced_with_return(self):
        self.assertIn("a04: 4770 bx lr", BACKLIGHT_ADDRESS_DISPATCH_BYPASS)

        with self.assertRaisesRegex(WitnessError, "address mapping.*indexed dispatch"):
            validate_call_path(BACKLIGHT_ADDRESS_DISPATCH_BYPASS)

    def test_rejects_conversion_dispatch_replaced_with_return(self):
        self.assertIn("954: 4770 bx lr", BACKLIGHT_CONVERSION_DISPATCH_BYPASS)

        with self.assertRaisesRegex(WitnessError, "conversion.*indexed dispatch"):
            validate_call_path(BACKLIGHT_CONVERSION_DISPATCH_BYPASS)

    def test_rejects_conversion_case_branch_to_another_register_case(self):
        self.assertIn("976: e7ff b.n 978", BACKLIGHT_CONVERSION_CASE_WRONG_RETURN)
        self.assertIn("978: a901 add r1", BACKLIGHT_CONVERSION_CASE_WRONG_RETURN)

        with self.assertRaisesRegex(WitnessError, "conversion cases do not share"):
            validate_call_path(BACKLIGHT_CONVERSION_CASE_WRONG_RETURN)

    def test_rejects_wrong_backlight_plan_operand(self):
        with self.assertRaisesRegex(WitnessError, "backlight plan operands"):
            validate_call_path(BACKLIGHT_WRONG_PLAN_OPERAND)

    def test_rejects_executor_iterator_with_wrong_plan_pointer(self):
        original = "85a: 9805 ldr r0, [sp, #20]"
        replacement = "85a: a80a add r0, sp, #40"
        self.assertIn(original, CONNECTED_DISPATCH)
        mutated = CONNECTED_DISPATCH.replace(original, replacement)
        self.assertIn(replacement, mutated)
        with self.assertRaisesRegex(WitnessError, "backlight executor"):
            validate_call_path(mutated)

    def test_rejects_iterator_result_clobber_before_return(self):
        original = "110e: bd80 pop {r7, pc}"
        replacement = "110e: 4601 mov r1, r0\n 1110: bd80 pop {r7, pc}"
        self.assertIn(original, CONNECTED_DISPATCH)
        mutated = CONNECTED_DISPATCH.replace(original, replacement)
        self.assertIn(replacement, mutated)
        with self.assertRaisesRegex(WitnessError, "backlight initialization.*writes"):
            validate_call_path(mutated)

    def test_rejects_register_io_without_emitted_prologue(self):
        mutations = (
            (" 920: b580 push {r7, lr}", " 920: 46c0 nop"),
            (" 2006: 9201 str r2, [sp, #4]", " 2006: 46c0 nop"),
        )
        for original, replacement in mutations:
            with self.subTest(replacement=replacement):
                self.assertIn(original, CONNECTED_DISPATCH)
                mutated = CONNECTED_DISPATCH.replace(original, replacement)
                with self.assertRaisesRegex(WitnessError, "backlight register.*continuation"):
                    validate_call_path(mutated)

    def test_rejects_compact_register_io_without_stack_setup(self):
        compact_read = """
 920: 4608 mov r0, r1
 922: f000 f800 bl 942 <RegisterAddress>::from_register>
 926: f000 f800 bl 9f2 <RegisterAddress>::address>
 92a: 6800 ldr r0, [r0, #0]
 92c: bd80 pop {r7, pc}
"""
        compact_write = """
 930: 4608 mov r0, r1
 932: f000 f800 bl 942 <RegisterAddress>::from_register>
 936: f000 f800 bl 9f2 <RegisterAddress>::address>
 93a: 9a01 ldr r2, [sp, #4]
 93c: 6002 str r2, [r0, #0]
 93e: bd80 pop {r7, pc}
"""
        for method, body in (("read", compact_read), ("write", compact_write)):
            with self.subTest(method=method):
                with self.assertRaisesRegex(WitnessError, "backlight register.*continuation"):
                    validate_backlight_register_io_path(body, method)

    def test_rejects_changed_plan_construction_and_copy(self):
        mutations = (
            ("1010: 0000 strb r4, [r5, #12]", "1010: 0000 strb r4, [r5, #8]"),
            ("str r3, [sp, #8]", "str r3, [sp, #4]"),
            ("str r2, [sp, #44]", "str r2, [sp, #40]"),
            ("str r1, [sp, #192]", "str r1, [sp, #188]"),
            ("add r1, sp, #100", "add r1, sp, #96"),
            ("movs r2, #96", "movs r2, #92"),
            ("bl 1200 <__aeabi_memcpy4>", "bl 1200 <other_copy>"),
            ("ldr r4, [pc, #168]", "ldr r4, [pc, #172]"),
            ("10cc: 000003ff .word 0x000003ff", "10cc: 000003fe .word 0x000003fe"),
        )
        for original, replacement in mutations:
            with self.subTest(replacement=replacement):
                self.assertEqual(_plan_body.count(original), 1)
                mutated = CONNECTED_DISPATCH.replace(
                    _plan_body, _plan_body.replace(original, replacement)
                )
                self.assertIn(replacement, mutated)
                with self.assertRaisesRegex(WitnessError, "backlight plan"):
                    validate_call_path(mutated)

    def test_rejects_wrong_backlight_iterator_dispatch_inputs(self):
        mutations = (
            ("892: 0081 lsls r1, r0, #2", "892: 2100 movs r1, #0"),
            ("87c: 9823 ldr r0, [sp, #140]", "87c: 9822 ldr r0, [sp, #136]"),
            ("87e: 9027 str r0, [sp, #156]", "87e: 9026 str r0, [sp, #152]"),
            ("880: 9822 ldr r0, [sp, #136]", "880: 9823 ldr r0, [sp, #140]"),
            ("882: 9026 str r0, [sp, #152]", "882: 9027 str r0, [sp, #156]"),
            ("884: 9821 ldr r0, [sp, #132]", "884: 9822 ldr r0, [sp, #136]"),
            ("886: 9025 str r0, [sp, #148]", "886: 9024 str r0, [sp, #144]"),
            ("888: 9820 ldr r0, [sp, #128]", "888: 9821 ldr r0, [sp, #132]"),
            ("88a: 9024 str r0, [sp, #144]", "88a: 9025 str r0, [sp, #148]"),
            ("88c: 9824 ldr r0, [sp, #144]", "88c: 9825 ldr r0, [sp, #148]"),
            ("88e: 9003 str r0, [sp, #12]", "88e: 9002 str r0, [sp, #8]"),
            ("890: 9803 ldr r0, [sp, #12]", "890: 9802 ldr r0, [sp, #8]"),
            ("892: 0081 lsls r1, r0, #2", "892: 0041 lsls r1, r0, #1"),
        )
        for original, replacement in mutations:
            with self.subTest(replacement=replacement):
                self.assertIn(original, CONNECTED_DISPATCH)
                disassembly = CONNECTED_DISPATCH.replace(original, replacement)
                self.assertIn(replacement, disassembly)
                with self.assertRaisesRegex(WitnessError, "backlight executor.*dispatch"):
                    validate_call_path(disassembly)

    def test_rejects_wrong_register_mapping_dispatch_inputs(self):
        for start in (0x942, 0x9f2):
            mutations = (
                (6, "b2c0 uxtb r0, r0", "b2c0 uxtb r0, r1"),
                (8, "9000 str r0, [sp, #0]", "9000 str r0, [sp, #4]"),
                (10, "9800 ldr r0, [sp, #0]", "9800 ldr r0, [sp, #4]"),
                (12, "0081 lsls r1, r0, #2", "0081 movs r1, #0"),
                (12, "0081 lsls r1, r0, #2", "0081 lsls r1, r0, #1"),
            )
            for offset, original_instruction, replacement_instruction in mutations:
                address = f"{start + offset:x}"
                original = f"{address}: {original_instruction}"
                changed = f"{address}: {replacement_instruction}"
                with self.subTest(address=address, replacement=replacement_instruction):
                    self.assertIn(original, CONNECTED_DISPATCH)
                    mutated = CONNECTED_DISPATCH.replace(original, changed)
                    self.assertIn(changed, mutated)
                    with self.assertRaisesRegex(WitnessError, "backlight register.*dispatch"):
                        validate_call_path(mutated)

    def test_rejects_missing_backlight_fresh_read(self):
        with self.assertRaisesRegex(
            WitnessError, "write case 0 has an unexpected register continuation"
        ):
            validate_call_path(BACKLIGHT_MISSING_FRESH_READ)

    def test_rejects_wrong_backlight_rmw_order(self):
        with self.assertRaisesRegex(
            WitnessError, "write case 0 has an unexpected register continuation"
        ):
            validate_call_path(BACKLIGHT_WRONG_RMW_ORDER)

    def test_rejects_backlight_rmw_value_clobber(self):
        self.assertIn("8c0: 2200 movs r2, #0", BACKLIGHT_RMW_VALUE_CLOBBER)

        with self.assertRaisesRegex(
            WitnessError, "write case 0 has an unexpected register continuation"
        ):
            validate_call_path(BACKLIGHT_RMW_VALUE_CLOBBER)

    def test_rejects_register_read_bypassing_mapping_and_mmio(self):
        self.assertIn("920: d007 beq.n 932", BACKLIGHT_REGISTER_READ_BYPASSES_ACCESS)
        self.assertIn("932: bd80 pop {r7, pc}", BACKLIGHT_REGISTER_READ_BYPASSES_ACCESS)

        with self.assertRaisesRegex(WitnessError, "register read.*continuation"):
            validate_call_path(BACKLIGHT_REGISTER_READ_BYPASSES_ACCESS)

    def test_rejects_register_write_bypassing_mapping_and_mmio(self):
        self.assertIn("2000: d00b beq.n 201a", BACKLIGHT_REGISTER_WRITE_BYPASSES_ACCESS)
        self.assertIn("201a: bd80 pop {r7, pc}", BACKLIGHT_REGISTER_WRITE_BYPASSES_ACCESS)

        with self.assertRaisesRegex(WitnessError, "register write.*continuation"):
            validate_call_path(BACKLIGHT_REGISTER_WRITE_BYPASSES_ACCESS)

    def test_rejects_initializer_returning_before_executor_call(self):
        self.assertIn("842: d001 beq.n 848", BACKLIGHT_INITIALIZER_EARLY_RETURN)
        self.assertIn("844: f000 f800 bl 846", BACKLIGHT_INITIALIZER_EARLY_RETURN)

        with self.assertRaisesRegex(WitnessError, "initializer.*continuation"):
            validate_backlight_initializer_path(BACKLIGHT_INITIALIZER_EARLY_RETURN)

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
