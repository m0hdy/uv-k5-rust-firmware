#!/usr/bin/env python3

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[2]
EXPECTED_COMMIT = "7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3"
FIXTURE = ROOT / "tests" / "reference-vectors" / "golden.json"
SUPPORT = Path(__file__).with_name("settings_capture_support.h")
DRIVER = Path(__file__).with_name("capture.c")
FEATURES = (
    "ENABLE_WIDE_RX",
    "ENABLE_VOX",
    "ENABLE_DTMF_CALLING",
    "ENABLE_RSSI_BAR",
    "ENABLE_AUDIO_BAR",
    "ENABLE_AM_FIX",
)


def checked_reference():
    reference = os.environ.get("UV_K5_C_REFERENCE")
    if not reference:
        raise SystemExit("set UV_K5_C_REFERENCE to a clean checkout at the pinned C commit")
    reference = Path(reference).resolve()
    actual = subprocess.check_output(
        ["git", "-C", str(reference), "rev-parse", "--verify", "HEAD"], text=True
    ).strip()
    if actual != EXPECTED_COMMIT:
        raise SystemExit(f"C reference must be at {EXPECTED_COMMIT} (found {actual})")
    status = subprocess.check_output(
        ["git", "-C", str(reference), "status", "--porcelain", "--untracked-files=all"],
        text=True,
    )
    if status:
        raise SystemExit(f"C reference must have a clean working tree:\n{status}")
    return reference


def extract_function(source, signature):
    start = source.index(signature)
    opening = source.index("{", start)
    depth = 0
    state = "code"
    index = opening
    while index < len(source):
        char = source[index]
        following = source[index + 1] if index + 1 < len(source) else ""
        if state == "code":
            if char == "/" and following == "*":
                state = "block_comment"
                index += 1
            elif char == "/" and following == "/":
                state = "line_comment"
                index += 1
            elif char == '"':
                state = "string"
            elif char == "'":
                state = "character"
            elif char == "{":
                depth += 1
            elif char == "}":
                depth -= 1
                if depth == 0:
                    return source[start : index + 1]
        elif state == "block_comment" and char == "*" and following == "/":
            state = "code"
            index += 1
        elif state == "line_comment" and char == "\n":
            state = "code"
        elif state in ("string", "character"):
            if char == "\\":
                index += 1
            elif (state == "string" and char == '"') or (
                state == "character" and char == "'"
            ):
                state = "code"
        index += 1
    raise SystemExit(f"could not extract {signature}")


def run_capture(reference):
    compiler = os.environ.get("CC", "cc")
    source = (reference / "settings.c").read_text(encoding="utf-8")
    settings_function = extract_function(source, "void SETTINGS_SaveSettings(void)")
    with tempfile.TemporaryDirectory(prefix="uvk5-reference-vectors-") as directory:
        directory = Path(directory)
        settings_source = directory / "settings_capture.c"
        settings_source.write_text(
            f'#include "{SUPPORT}"\n{settings_function}\n', encoding="utf-8"
        )
        binary = directory / "capture"
        command = [
            compiler,
            "-std=gnu2x",
            "-I",
            str(reference),
            "-I",
            str(SUPPORT.parent),
            "-include",
            str(SUPPORT),
            *[f"-D{feature}" for feature in FEATURES],
            str(DRIVER),
            str(settings_source),
            str(reference / "dcs.c"),
            str(reference / "frequencies.c"),
            "-o",
            str(binary),
        ]
        subprocess.run(command, check=True)
        output = subprocess.check_output([str(binary)], text=True)

    cases = []
    settings = {}
    for line in output.splitlines():
        category, name, input_value, expected = line.split("\t")
        if category == "settings":
            settings.setdefault(name, {})[input_value] = expected
            continue
        cases.append(
            {
                "category": category,
                "name": name,
                "input": input_value,
                "expected": int(expected),
            }
        )
    for profile, writes in settings.items():
        cases.append(
            {
                "category": "settings",
                "name": profile,
                "input": {
                    "profile": profile,
                    "feature_defines": list(FEATURES),
                    "values": "see capture_settings() in tools/reference-vectors/capture.c",
                },
                "expected": writes,
            }
        )
    cases.append(capture_packer(reference))
    return cases


def capture_packer(reference):
    payload = bytes(range(256)) * 48
    with tempfile.TemporaryDirectory(prefix="uvk5-packer-vector-") as directory:
        directory = Path(directory)
        input_path = directory / "input.bin"
        output_path = directory / "packed.bin"
        input_path.write_bytes(payload)
        try:
            subprocess.run(
                [
                    sys.executable,
                    str(reference / "fw-pack.py"),
                    str(input_path),
                    "Golden",
                    "1.0",
                    str(output_path),
                ],
                check=True,
                capture_output=True,
                text=True,
            )
        except subprocess.CalledProcessError as error:
            raise SystemExit(
                "pinned fw-pack.py failed; install its crcmod dependency "
                "(the recorded capture used crcmod 1.7)"
            ) from error
        packed = output_path.read_bytes()
    return {
        "category": "firmware_container_checksum",
        "name": "byte_ramp_12k",
        "input": {
            "raw_pattern": "bytes 0x00..0xff repeated 48 times",
            "raw_length": len(payload),
            "author": "Golden",
            "version": "1.0",
        },
        "expected": {
            "raw_sha256": hashlib.sha256(payload).hexdigest(),
            "packed_length": len(packed),
            "packed_sha256": hashlib.sha256(packed).hexdigest(),
            "appended_crc_bytes": packed[-2:].hex(),
        },
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    if args.check and args.write:
        parser.error("--check and --write are mutually exclusive")

    reference = checked_reference()
    cases = run_capture(reference)
    result = {"reference_commit": EXPECTED_COMMIT, "cases": cases}
    if args.write:
        FIXTURE.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    elif args.check:
        expected = json.loads(FIXTURE.read_text(encoding="utf-8"))
        if expected != result:
            raise SystemExit("captured values differ from tests/reference-vectors/golden.json")
        print(f"verified {len(cases)} golden cases against the pinned C source")
    else:
        print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
