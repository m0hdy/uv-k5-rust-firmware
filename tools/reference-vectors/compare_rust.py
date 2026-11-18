#!/usr/bin/env python3

import subprocess
import sys

from runner import Category, ROOT, VectorFormatError, format_report, load_fixture, run_vectors


SUPPORTED_CATEGORIES = {
    Category.CTCSS,
    Category.DCS,
    Category.FREQUENCY,
    Category.SETTINGS,
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
UNSUPPORTED_REASONS = {
    Category.CHANNEL: (
        "the Rust library exposes no equivalent to RADIO_CheckValidChannel; "
        "scan-decision behavior is not a substitute"
    ),
    Category.FIRMWARE_CONTAINER_CHECKSUM: (
        "the Rust library has no updater-container packer/checksum API; the fixture "
        "covers a packed image and appended CRC, not an EEPROM/settings checksum"
    ),
}


def evaluate_rust(fixture):
    requests = []
    expected_ids = set()
    for case in fixture.cases:
        if case.category not in SUPPORTED_CATEGORIES:
            continue
        expected_ids.add(case.vector_id)
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
    for category, reason in UNSUPPORTED_REASONS.items():
        case_ids = report.unported.get(category, ())
        if case_ids:
            print(f"Unsupported {category.value}: {reason}")
    return 0 if report.complete else 1


if __name__ == "__main__":
    raise SystemExit(main())
