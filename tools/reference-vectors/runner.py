#!/usr/bin/env python3

from copy import deepcopy
from dataclasses import dataclass
from enum import Enum
import json
from pathlib import Path
import re
from typing import Callable, TypeAlias


ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "tests" / "reference-vectors" / "golden.json"
REFERENCE_COMMIT = "7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3"
SHA256 = re.compile(r"[0-9a-f]{64}\Z")

JsonValue: TypeAlias = None | bool | int | float | str | list | dict
Evaluator: TypeAlias = Callable[["VectorCase"], JsonValue]


class VectorFormatError(ValueError):
    pass


class Category(str, Enum):
    CHANNEL = "channel"
    CTCSS = "ctcss"
    DCS = "dcs"
    FREQUENCY = "frequency"
    SETTINGS = "settings"
    FIRMWARE_CONTAINER_CHECKSUM = "firmware_container_checksum"


SCALAR_CATEGORIES = {
    Category.CHANNEL,
    Category.CTCSS,
    Category.DCS,
    Category.FREQUENCY,
}


@dataclass(frozen=True)
class VectorCase:
    category: Category
    name: str
    input: str | dict[str, object]
    expected: int | dict[str, object]

    @property
    def vector_id(self) -> str:
        return f"{self.category.value}/{self.name}"


@dataclass(frozen=True)
class VectorFixture:
    reference_commit: str
    cases: tuple[VectorCase, ...]


@dataclass(frozen=True)
class Mismatch:
    vector_id: str
    expected: JsonValue
    actual: JsonValue


@dataclass(frozen=True)
class RunReport:
    total: int
    matched: int
    mismatches: tuple[Mismatch, ...]
    unported: dict[Category, tuple[str, ...]]

    @property
    def complete(self) -> bool:
        return self.matched == self.total and not self.mismatches and not self.unported


def _object_without_duplicate_keys(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise VectorFormatError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def _require_object(value, fields, context):
    if not isinstance(value, dict):
        raise VectorFormatError(f"{context} must be an object")
    missing = fields - value.keys()
    extra = value.keys() - fields
    if missing or extra:
        details = []
        if missing:
            details.append(f"missing fields: {', '.join(sorted(missing))}")
        if extra:
            details.append(f"unexpected fields: {', '.join(sorted(extra))}")
        raise VectorFormatError(f"{context}: {'; '.join(details)}")
    return value


def _validate_scalar_input(value, category, context):
    if not isinstance(value, str) or not value:
        raise VectorFormatError(f"{context} must be a non-empty key=value string")
    assignments = {}
    for assignment in value.split(","):
        key, separator, item = assignment.partition("=")
        if not separator or not key or not item or key in assignments:
            raise VectorFormatError(f"{context} contains an invalid assignment {assignment!r}")
        assignments[key] = item

    schemas = {
        Category.CHANNEL: (
            {
                "channel", "band", "scanlist1", "scanlist2",
                "check", "list", "priority1", "priority2",
            },
        ),
        Category.CTCSS: ({"tenths_hz"},),
        Category.DCS: ({"type", "option"}, {"word"}),
        Category.FREQUENCY: ({"ticks"}, {"lock", "ticks"}, {"ticks", "step"}),
    }
    allowed_schemas = schemas[category]
    if set(assignments) not in allowed_schemas:
        expected = " or ".join(", ".join(sorted(schema)) for schema in allowed_schemas)
        raise VectorFormatError(
            f"{context} has keys for the wrong {category.value} operation; expected {expected}"
        )

    for key, item in assignments.items():
        if key == "lock":
            if item not in {"default", "fcc", "none", "all"}:
                raise VectorFormatError(f"{context}.{key} must be a supported lock mode")
        elif not re.fullmatch(r"-?[0-9]+", item):
            raise VectorFormatError(f"{context}.{key} must be an integer")
    if category is Category.DCS and "type" in assignments:
        if assignments["type"] not in {"2", "3"}:
            raise VectorFormatError(f"{context}.type must be 2 or 3")
    if category is Category.FREQUENCY and "step" in assignments:
        if int(assignments["step"]) <= 0:
            raise VectorFormatError(f"{context}.step must be a positive integer")


def _validate_settings_case(input_value, expected, context):
    _require_object(input_value, {"profile", "feature_defines", "values"}, f"{context}.input")
    if not isinstance(input_value["profile"], str) or not input_value["profile"]:
        raise VectorFormatError(f"{context}.input.profile must be a non-empty string")
    features = input_value["feature_defines"]
    if not isinstance(features, list) or not all(
        isinstance(feature, str) and feature for feature in features
    ):
        raise VectorFormatError(f"{context}.input.feature_defines must be a list of strings")
    if len(set(features)) != len(features):
        raise VectorFormatError(f"{context}.input.feature_defines must not contain duplicates")
    if not isinstance(input_value["values"], str) or not input_value["values"]:
        raise VectorFormatError(f"{context}.input.values must be a non-empty string")
    if not isinstance(expected, dict) or not expected:
        raise VectorFormatError(f"{context}.expected must be an address-to-bytes object")
    for address, encoded in expected.items():
        if not isinstance(address, str) or not re.fullmatch(r"0x[0-9a-f]{4}", address):
            raise VectorFormatError(f"{context}.expected has invalid EEPROM address {address!r}")
        if not isinstance(encoded, str) or not re.fullmatch(r"[0-9a-f]{16}", encoded):
            raise VectorFormatError(
                f"{context}.expected[{address!r}] must encode exactly eight bytes"
            )


def _validate_checksum_case(input_value, expected, context):
    _require_object(
        input_value,
        {"raw_pattern", "raw_length", "author", "version"},
        f"{context}.input",
    )
    for field in ("raw_pattern", "author", "version"):
        if not isinstance(input_value[field], str) or not input_value[field]:
            raise VectorFormatError(f"{context}.input.{field} must be a non-empty string")
    if type(input_value["raw_length"]) is not int or input_value["raw_length"] <= 0:
        raise VectorFormatError(f"{context}.input.raw_length must be a positive integer")
    _require_object(
        expected,
        {"raw_sha256", "packed_length", "packed_sha256", "appended_crc_bytes"},
        f"{context}.expected",
    )
    for field in ("raw_sha256", "packed_sha256"):
        if not isinstance(expected[field], str) or not SHA256.fullmatch(expected[field]):
            raise VectorFormatError(f"{context}.expected.{field} must be a SHA-256 hex digest")
    if type(expected["packed_length"]) is not int or expected["packed_length"] <= 0:
        raise VectorFormatError(f"{context}.expected.packed_length must be a positive integer")
    crc_bytes = expected["appended_crc_bytes"]
    if not isinstance(crc_bytes, str) or not re.fullmatch(r"[0-9a-f]{4}", crc_bytes):
        raise VectorFormatError(f"{context}.expected.appended_crc_bytes must encode two bytes")


def parse_fixture(value: object) -> VectorFixture:
    root = _require_object(value, {"reference_commit", "cases"}, "fixture")
    if root["reference_commit"] != REFERENCE_COMMIT:
        raise VectorFormatError(
            f"fixture reference_commit must be the approved {REFERENCE_COMMIT}"
        )
    if not isinstance(root["cases"], list) or not root["cases"]:
        raise VectorFormatError("fixture.cases must be a non-empty list")

    cases = []
    seen = set()
    for index, value in enumerate(root["cases"]):
        context = f"fixture.cases[{index}]"
        item = _require_object(value, {"category", "name", "input", "expected"}, context)
        try:
            category = Category(item["category"])
        except (ValueError, TypeError):
            raise VectorFormatError(f"{context}.category is not a supported vector domain") from None
        name = item["name"]
        if not isinstance(name, str) or not name or not re.fullmatch(r"[a-z0-9_]+", name):
            raise VectorFormatError(f"{context}.name must be a non-empty snake_case identifier")
        vector_id = f"{category.value}/{name}"
        if vector_id in seen:
            raise VectorFormatError(f"duplicate vector ID {vector_id}")
        seen.add(vector_id)

        input_value, expected = item["input"], item["expected"]
        if category in SCALAR_CATEGORIES:
            _validate_scalar_input(input_value, category, f"{context}.input")
            if type(expected) is not int:
                raise VectorFormatError(f"{context}.expected must be an integer")
        elif category is Category.SETTINGS:
            _validate_settings_case(input_value, expected, context)
        else:
            _validate_checksum_case(input_value, expected, context)
        cases.append(VectorCase(category, name, input_value, expected))

    return VectorFixture(root["reference_commit"], tuple(cases))


def load_fixture(path: Path = FIXTURE) -> VectorFixture:
    try:
        raw = json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=_object_without_duplicate_keys,
        )
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise VectorFormatError(f"cannot read vector fixture {path}: {error}") from error
    return parse_fixture(raw)


def run_vectors(
    fixture: VectorFixture, evaluators: dict[Category, Evaluator]
) -> RunReport:
    unsupported = evaluators.keys() - set(Category)
    if unsupported:
        raise ValueError(f"evaluators provided for unknown domains: {unsupported}")

    mismatches = []
    unported: dict[Category, list[str]] = {}
    matched = 0
    for case in fixture.cases:
        evaluator = evaluators.get(case.category)
        if evaluator is None:
            unported.setdefault(case.category, []).append(case.vector_id)
            continue
        actual = evaluator(deepcopy(case))
        if _type_sensitive_equal(actual, case.expected):
            matched += 1
        else:
            mismatches.append(
                Mismatch(case.vector_id, deepcopy(case.expected), deepcopy(actual))
            )
    return RunReport(
        len(fixture.cases),
        matched,
        tuple(mismatches),
        {category: tuple(ids) for category, ids in unported.items()},
    )


def _type_sensitive_equal(actual, expected):
    if type(actual) is not type(expected):
        return False
    if isinstance(expected, dict):
        return len(actual) == len(expected) and all(
            any(
                type(actual_key) is type(expected_key)
                and actual_key == expected_key
                and _type_sensitive_equal(actual_value, expected_value)
                for actual_key, actual_value in actual.items()
            )
            for expected_key, expected_value in expected.items()
        )
    if isinstance(expected, list):
        return len(actual) == len(expected) and all(
            _type_sensitive_equal(actual_item, expected_item)
            for actual_item, expected_item in zip(actual, expected)
        )
    return actual == expected


def _format_value(value: JsonValue) -> str:
    return json.dumps(value, separators=(",", ":"))


def format_report(report: RunReport) -> str:
    executed = report.matched + len(report.mismatches)
    lines = [f"Vector results: {report.matched}/{report.total} matched ({executed} executed)."]
    for mismatch in report.mismatches:
        lines.append(
            f"Mismatch {mismatch.vector_id}: expected {_format_value(mismatch.expected)}, "
            f"actual {_format_value(mismatch.actual)}"
        )
    if report.unported:
        domains = ", ".join(
            f"{category.value} ({len(case_ids)} case{'s' if len(case_ids) != 1 else ''})"
            for category, case_ids in sorted(
                report.unported.items(), key=lambda item: item[0].value
            )
        )
        lines.append(f"Unported domains (not executed): {domains}")
        for category, case_ids in sorted(
            report.unported.items(), key=lambda item: item[0].value
        ):
            lines.append(f"  {category.value}: {', '.join(case_ids)}")
    if executed == 0:
        lines.append("No vector cases were executed; this is not parity coverage.")
    elif report.unported:
        lines.append("Unported cases are not counted as coverage.")
    return "\n".join(lines)


def main() -> int:
    try:
        fixture = load_fixture()
        report = run_vectors(fixture, {})
    except VectorFormatError as error:
        print(f"Invalid reference-vector fixture: {error}")
        return 2
    print(format_report(report))
    return 0 if report.complete else 1


if __name__ == "__main__":
    raise SystemExit(main())
