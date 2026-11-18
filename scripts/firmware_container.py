#!/usr/bin/env python3
"""Pack, validate, and unpack the updater container from the pinned C reference."""

import argparse
import binascii
from dataclasses import dataclass
from pathlib import Path
import sys


VERSION_OFFSET = 0x2000
VERSION_SIZE = 16
CRC_SIZE = 2
# Modified stdlib integration; table reproduced from upstream fw-pack.py at commit
# 7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3.
OBFUSCATION = bytes.fromhex(
    "47 22 c0 52 5d 57 48 94 b1 60 60 db 6f e3 4c 7c "
    "d8 4a d6 8b 30 ec 25 e0 4c d9 00 7f bf e3 54 05 "
    "e9 3a 97 6b b0 6e 0c fb b1 1a e2 c9 c1 56 47 e9 "
    "ba f1 42 b6 67 5f 0f 96 f7 c9 3c 84 1b 26 e1 4e "
    "3b 6f 66 e6 a0 6a b0 bf c6 a5 70 3a ba 18 9e 27 "
    "1a 53 5b 71 b1 94 1e 18 f2 d6 81 02 22 fd 5a 28 "
    "91 db ba 5d 64 c6 fe 86 83 9c 50 1c 73 03 11 d6 "
    "af 30 f4 2c 77 b2 7d bb 3f 29 28 57 22 d6 92 8b"
)


class ContainerError(ValueError):
    """Raised when a container is malformed or fails validation."""


@dataclass(frozen=True)
class ContainerContents:
    payload: bytes
    version_field: bytes
    version_offset: int


def _version_field(author, version):
    try:
        field = b"*" + author.encode("ascii") + b" " + version.encode("ascii")
    except UnicodeEncodeError as error:
        raise ContainerError("author and version must contain only ASCII characters") from error
    return field[:VERSION_SIZE].ljust(VERSION_SIZE, b"\0")


def _xor_obfuscation(data):
    return bytes(
        value ^ OBFUSCATION[index % len(OBFUSCATION)]
        for index, value in enumerate(data)
    )


def _crc_bytes(data):
    crc = binascii.crc_hqx(data, 0)
    return bytes((crc & 0xFF, crc >> 8))


def pack_payload(payload, author, version):
    """Return a packed container matching the pinned fw-pack.py output."""
    field = _version_field(author, version)
    transformed = payload[:VERSION_OFFSET] + field + payload[VERSION_OFFSET:]
    obfuscated = _xor_obfuscation(transformed)
    return obfuscated + _crc_bytes(obfuscated)


def decode_container(container, author=None, version=None):
    """Validate a container and return its raw payload and decoded version field."""
    if (author is None) != (version is None):
        raise ContainerError("author and version must be supplied together")
    if len(container) < VERSION_SIZE + CRC_SIZE:
        raise ContainerError("container is too short to contain a version field and CRC")

    obfuscated = container[:-CRC_SIZE]
    if container[-CRC_SIZE:] != _crc_bytes(obfuscated):
        raise ContainerError("container CRC-XMODEM mismatch")

    transformed = _xor_obfuscation(obfuscated)
    version_offset = (
        VERSION_OFFSET
        if len(transformed) >= VERSION_OFFSET + VERSION_SIZE
        else len(transformed) - VERSION_SIZE
    )
    field = transformed[version_offset : version_offset + VERSION_SIZE]
    if field[:1] != b"*":
        raise ContainerError("container version field marker is missing")
    if not field.isascii():
        raise ContainerError("container version field must contain only ASCII characters")
    if author is not None and field != _version_field(author, version):
        raise ContainerError(
            "container version field does not match the requested author/version"
        )

    payload = transformed[:version_offset] + transformed[version_offset + VERSION_SIZE :]
    return ContainerContents(payload, field, version_offset)


def _add_expected_version_arguments(command):
    command.add_argument("--author")
    command.add_argument("--version")


def _expected_version(arguments):
    if (arguments.author is None) != (arguments.version is None):
        raise ContainerError("--author and --version must be supplied together")
    return arguments.author, arguments.version


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)

    pack_parser = commands.add_parser("pack", help="pack a raw firmware payload")
    pack_parser.add_argument("input", type=Path)
    pack_parser.add_argument("output", type=Path)
    pack_parser.add_argument("--author", required=True)
    pack_parser.add_argument("--version", required=True)

    validate_parser = commands.add_parser("validate", help="validate a packed container")
    validate_parser.add_argument("input", type=Path)
    _add_expected_version_arguments(validate_parser)

    unpack_parser = commands.add_parser("unpack", help="validate and recover a raw payload")
    unpack_parser.add_argument("input", type=Path)
    unpack_parser.add_argument("output", type=Path)
    _add_expected_version_arguments(unpack_parser)

    arguments = parser.parse_args(argv)
    try:
        if arguments.command == "pack":
            packed = pack_payload(
                arguments.input.read_bytes(), arguments.author, arguments.version
            )
            arguments.output.write_bytes(packed)
            print(f"packed {arguments.input} -> {arguments.output} ({len(packed)} bytes)")
        else:
            author, version = _expected_version(arguments)
            contents = decode_container(
                arguments.input.read_bytes(), author=author, version=version
            )
            if arguments.command == "unpack":
                arguments.output.write_bytes(contents.payload)
                print(
                    f"unpacked {arguments.input} -> {arguments.output} "
                    f"({len(contents.payload)} bytes)"
                )
            else:
                print(
                    f"valid container: {len(contents.payload)} payload bytes, "
                    f"version field {contents.version_field!r}"
                )
    except (OSError, ContainerError) as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    sys.exit(main())
