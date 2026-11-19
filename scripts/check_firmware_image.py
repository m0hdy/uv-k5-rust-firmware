#!/usr/bin/env python3
"""Gate a raw and packed development image against its ELF and linker map."""

import argparse
from pathlib import Path
import struct
import sys

from check_memory_budget import (
    BudgetError,
    REQUIRED_STACK_RESERVATION,
    analyze,
    format_report,
)
from check_startup_layout import StartupError, validate
from firmware_container import ContainerError, decode_container


class ImageValidationError(ValueError):
    pass


def validate_image(
    elf_path: Path,
    map_path: Path,
    raw_path: Path,
    packed_path: Path,
    author: str,
    version: str,
) -> str:
    validate(elf_path, map_path)
    report = analyze(
        elf_path, raw_path, REQUIRED_STACK_RESERVATION, packed_path
    )
    budget_report = format_report(report, REQUIRED_STACK_RESERVATION)
    if report.errors:
        raise ImageValidationError(budget_report)

    raw_payload = raw_path.read_bytes()
    container = decode_container(
        packed_path.read_bytes(), author=author, version=version
    )
    if container.payload != raw_payload:
        raise ImageValidationError(
            "packed container payload does not match the raw image"
        )

    return (
        "Startup layout valid: entry/reset vector, initial SP, 48 vectors, "
        "sections, flash/RAM ranges, and linker map\n"
        f"{budget_report}\n"
        "Container valid: author/version and CRC match\n"
        "Raw and packed payloads match"
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--elf", required=True, type=Path)
    parser.add_argument("--map", required=True, type=Path)
    parser.add_argument("--raw", required=True, type=Path)
    parser.add_argument("--packed", required=True, type=Path)
    parser.add_argument("--author", required=True)
    parser.add_argument("--version", required=True)
    arguments = parser.parse_args(argv)

    try:
        print(
            validate_image(
                arguments.elf,
                arguments.map,
                arguments.raw,
                arguments.packed,
                arguments.author,
                arguments.version,
            )
        )
    except (
        OSError,
        struct.error,
        BudgetError,
        StartupError,
        ContainerError,
        ImageValidationError,
    ) as error:
        print(f"Firmware image gate failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
