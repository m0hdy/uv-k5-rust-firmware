#!/usr/bin/env python3

import hashlib
import subprocess
import sys

from runner import Category, ROOT, VectorFormatError, format_report, load_fixture, run_vectors


SUPPORTED_CATEGORIES = {
    Category.CHANNEL,
    Category.CTCSS,
    Category.DCS,
    Category.FREQUENCY,
    Category.SETTINGS,
    Category.FIRMWARE_CONTAINER_CHECKSUM,
}
PINNED_FEATURE_DEFINES = [
    "ENABLE_WIDE_RX",
    "ENABLE_VOX",
    "ENABLE_DTMF_CALLING",
    "ENABLE_RSSI_BAR",
    "ENABLE_AUDIO_BAR",
    "ENABLE_AM_FIX",
]
SETTINGS_VALUE_DESCRIPTION = "see capture_settings() in tools/reference-vectors/capture.c"


def evaluate_rust(fixture):
    requests = []
    expected_ids = set()
    for case in fixture.cases:
        if case.category not in SUPPORTED_CATEGORIES:
            continue
        expected_ids.add(case.vector_id)
        if case.category is Category.FIRMWARE_CONTAINER_CHECKSUM:
            values = (
                case.input["raw_pattern"],
                str(case.input["raw_length"]),
                case.input["author"],
                case.input["version"],
            )
            if any("\t" in value or "\n" in value for value in values):
                raise RuntimeError(f"{case.vector_id} input contains a bridge delimiter")
            requests.append("\t".join((case.vector_id, *values)))
            continue
        if case.category is Category.SETTINGS:
            if (
                case.input["feature_defines"] != PINNED_FEATURE_DEFINES
                or case.input["values"] != SETTINGS_VALUE_DESCRIPTION
            ):
                raise RuntimeError(
                    f"{case.vector_id} input does not match the supported pinned settings profile"
                )
            input_fields = (case.input["profile"],)
        else:
            input_fields = (case.input,)
        requests.append("\t".join((case.vector_id, *input_fields)))

    completed = subprocess.run(
        ["cargo", "run", "--offline", "--quiet", "--example", "reference_vectors"],
        cwd=ROOT,
        input="".join(f"{request}\n" for request in requests),
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode:
        raise RuntimeError(
            "Rust reference-vector bridge failed:\n"
            f"{completed.stderr.strip() or completed.stdout.strip()}"
        )

    actuals = {}
    for line in completed.stdout.splitlines():
        vector_id, separator, encoded = line.partition("\t")
        if not separator or vector_id in actuals:
            raise RuntimeError(f"invalid or duplicate Rust bridge result: {line!r}")
        if vector_id not in expected_ids:
            raise RuntimeError(f"unexpected Rust bridge result: {vector_id}")
        if vector_id.startswith("settings/"):
            result = {}
            for block in encoded.split(","):
                address, separator, data = block.partition("=")
                if not separator:
                    raise RuntimeError(f"invalid Rust settings result: {block!r}")
                result[address] = data
        elif vector_id.startswith(f"{Category.FIRMWARE_CONTAINER_CHECKSUM.value}/"):
            raw_hex, separator, packed_hex = encoded.partition("\t")
            if not separator:
                raise RuntimeError(f"invalid Rust container result: {encoded!r}")
            try:
                raw, packed = bytes.fromhex(raw_hex), bytes.fromhex(packed_hex)
            except ValueError as error:
                raise RuntimeError(f"invalid Rust container bytes: {encoded!r}") from error
            if raw.hex() != raw_hex or packed.hex() != packed_hex or len(packed) < 2:
                raise RuntimeError(f"noncanonical Rust container bytes: {vector_id}")
            result = {
                "raw_sha256": hashlib.sha256(raw).hexdigest(),
                "packed_length": len(packed),
                "packed_sha256": hashlib.sha256(packed).hexdigest(),
                "appended_crc_bytes": packed[-2:].hex(),
            }
        else:
            try:
                result = int(encoded)
            except ValueError as error:
                raise RuntimeError(f"invalid Rust scalar result: {encoded!r}") from error
        actuals[vector_id] = result

    missing = expected_ids - actuals.keys()
    if missing:
        raise RuntimeError(f"Rust bridge omitted supported vectors: {', '.join(sorted(missing))}")
    return actuals


def compare_rust(fixture, actuals):
    evaluators = {
        category: lambda case: actuals[case.vector_id]
        for category in SUPPORTED_CATEGORIES
    }
    return run_vectors(fixture, evaluators)


def main():
    try:
        fixture = load_fixture()
        actuals = evaluate_rust(fixture)
        report = compare_rust(fixture, actuals)
    except (OSError, RuntimeError, VectorFormatError) as error:
        print(f"Rust reference-vector comparison failed: {error}", file=sys.stderr)
        return 2

    print(format_report(report))
    print(f"Rust-executed vectors: {', '.join(sorted(actuals))}")
    return 0 if report.complete else 1


if __name__ == "__main__":
    raise SystemExit(main())
