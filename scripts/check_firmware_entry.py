#!/usr/bin/env python3
"""Inspect the selected Cortex-M0 firmware entry and its reachable boot stages."""

import argparse
import re
import subprocess
import sys
from pathlib import Path


class EntryError(ValueError):
    pass


FUNCTION = re.compile(r"^\s*[0-9a-fA-F]+\s+<(.+)>:\s*$", re.MULTILINE)


def function_body(disassembly: str, symbol: str) -> str:
    functions = list(FUNCTION.finditer(disassembly))
    for index, match in enumerate(functions):
        if match.group(1) == symbol:
            end = functions[index + 1].start() if index + 1 < len(functions) else len(disassembly)
            return disassembly[match.end() : end]
    raise EntryError(f"disassembly is missing {symbol}")


def function_body_containing(disassembly: str, symbol_fragment: str) -> str:
    functions = list(FUNCTION.finditer(disassembly))
    matches = [
        index
        for index, match in enumerate(functions)
        if symbol_fragment in match.group(1)
    ]
    if len(matches) != 1:
        raise EntryError(
            f"disassembly must contain exactly one function matching {symbol_fragment}"
        )
    index = matches[0]
    end = functions[index + 1].start() if index + 1 < len(functions) else len(disassembly)
    return disassembly[functions[index].end() : end]


def require_call(body: str, target: str) -> None:
    if not any(
        re.search(r"\bbl(?:\.n|\.w)?\b", line) and target in line
        for line in body.splitlines()
    ):
        raise EntryError(f"entry path does not call {target}")


def inspect_disassembly(disassembly: str, symbols: str) -> None:
    reset = function_body(disassembly, "Reset_Handler")
    main = function_body(disassembly, "firmware_main")
    entry = function_body(disassembly, "firmware_integration_entry")
    coordinator = function_body_containing(disassembly, "coordinate_with_welcome::<")
    fail_closed = function_body(disassembly, "firmware_fail_closed")
    systick = function_body(disassembly, "SysTick_Handler")

    require_call(reset, "firmware_main")
    require_call(main, "firmware_integration_entry")
    require_call(entry, "firmware_fail_closed")
    require_call(entry, "coordinate_with_welcome::<")
    if not re.search(r"\bbx\s+lr\b", systick):
        raise EntryError("selected vector table's SysTick_Handler is not a valid return handler")
    if not re.search(r"\bcpsid\s+i\b", fail_closed) or not re.search(r"\bwfi\b", fail_closed):
        raise EntryError("firmware_fail_closed must mask interrupts and park")

    for stage in (
        "StartupIo>::systick",
        "StartupIo>::portcon",
        "StartupIo>::gpio",
        "StartupIo>::backlight_init_hardware",
        "StartupIo>::adc_init",
        "StartupIo>::st7565_init",
    ):
        require_call(coordinator, stage)
    if coordinator.count("eeprom_read_transport::execute_read::<") != 3:
        raise EntryError("firmware entry must execute the three fallible EEPROM settings reads")
    for executor in (
        "adc_samples::decode_adc_sample",
        "adc_samples::calibrated_battery_voltage",
        "startup_version_display::startup_version_display_plan",
        "startup_version_row_plan::compose_startup_version_row",
    ):
        require_call(coordinator, executor)
    if coordinator.count("StartupIo>::st7565_init") != 1:
        raise EntryError("firmware entry must initialize the board display exactly once")
    if "GpioI2cIo<" not in symbols or "systick_delay_adapter::SysTickDelay<" not in symbols:
        raise EntryError("selected entry lacks the concrete GPIO I2C/SysTick delay backend")
    if "0xe000e180" not in disassembly or "0xe000e100" in disassembly:
        raise EntryError("SARADC must remain masked without an NVIC enable write")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--elf", required=True, type=Path)
    parser.add_argument(
        "--objdump",
        default=f"{__import__('os').environ.get('ARM_BINUTILS_PREFIX', 'arm-none-eabi-')}objdump",
    )
    args = parser.parse_args()

    try:
        disassembly = subprocess.run(
            [args.objdump, "-d", "-C", "--wide", str(args.elf)],
            check=True,
            capture_output=True,
            text=True,
        ).stdout
        symbols = subprocess.run(
            [args.objdump, "-t", "-C", "--wide", str(args.elf)],
            check=True,
            capture_output=True,
            text=True,
        ).stdout
        inspect_disassembly(disassembly, symbols)
    except (OSError, subprocess.CalledProcessError, EntryError) as error:
        print(f"firmware entry check failed: {error}", file=sys.stderr)
        return 1

    print("Selected Cortex-M0 entry reaches board, EEPROM, battery, and welcome executors")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
