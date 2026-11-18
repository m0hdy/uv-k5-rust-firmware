#!/usr/bin/env python3

import json
import sys

import capture


def compare_cases(canonical_cases, captured_cases):
    mismatches = []
    canonical = index_cases(canonical_cases, "canonical", mismatches)
    captured = index_cases(captured_cases, "captured", mismatches)

    for key in sorted(canonical.keys() - captured.keys()):
        mismatches.append(f"{format_key(key)}: missing captured case")
    for key in sorted(captured.keys() - canonical.keys()):
        mismatches.append(f"{format_key(key)}: unexpected captured case")

    for key in sorted(canonical.keys() & captured.keys()):
        expected = canonical[key]
        actual = captured[key]
        label = format_key(key)
        if expected["input"] != actual["input"]:
            mismatches.append(f"{label}: input mismatch")
        if expected["expected"] != actual["expected"]:
            mismatches.append(
                f"{label}: result mismatch "
                f"(canonical {expected['expected']!r}, captured {actual['expected']!r})"
            )
    return mismatches


def index_cases(cases, source, mismatches):
    indexed = {}
    for case in cases:
        key = (case["category"], case["name"])
        if key in indexed:
            mismatches.append(f"{format_key(key)}: duplicate {source} case")
        else:
            indexed[key] = case
    return indexed


def format_key(key):
    return f"{key[0]}/{key[1]}"


def main():
    reference = capture.checked_reference()
    canonical = json.loads(capture.FIXTURE.read_text(encoding="utf-8"))
    if canonical.get("reference_commit") != capture.EXPECTED_COMMIT:
        print(
            "canonical vectors do not use the pinned C reference revision "
            f"{capture.EXPECTED_COMMIT}",
            file=sys.stderr,
        )
        return 1

    captured_cases = capture.run_capture(reference)
    mismatches = compare_cases(canonical["cases"], captured_cases)
    if mismatches:
        print("pinned C comparison failed:", file=sys.stderr)
        for mismatch in mismatches:
            print(f"- {mismatch}", file=sys.stderr)
        return 1

    print(
        f"matched {len(captured_cases)} canonical cases against pinned C "
        f"reference {capture.EXPECTED_COMMIT}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
