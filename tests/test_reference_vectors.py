import hashlib
import json
from copy import deepcopy
from pathlib import Path
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools" / "reference-vectors"))
sys.path.insert(0, str(ROOT / "scripts"))

from runner import (  # noqa: E402
    Category,
    VectorFormatError,
    format_report,
    load_fixture,
    parse_fixture,
    run_vectors,
)
from compare_rust import compare_rust, evaluate_rust  # noqa: E402
from firmware_container import pack_payload  # noqa: E402


class ReferenceVectorRunnerTests(unittest.TestCase):
    def setUp(self):
        self.fixture = load_fixture()

    def test_checked_in_fixture_loads_without_c_reference(self):
        self.assertEqual(len(self.fixture.cases), 39)
        self.assertEqual(
            self.fixture.reference_commit,
            "7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3",
        )

    def test_rust_bridge_executes_all_vectors_and_rejects_changed_expectations(self):
        actuals = evaluate_rust(self.fixture)
        report = compare_rust(self.fixture, actuals)

        self.assertEqual(report.total, 39)
        self.assertEqual(report.matched, 39)
        self.assertEqual(report.mismatches, ())
        self.assertEqual(report.unported, {})
        self.assertTrue(report.complete)

        changed = json.loads((ROOT / "tests/reference-vectors/golden.json").read_text())
        changed_case = next(case for case in changed["cases"] if case["category"] == "channel")
        changed_case["expected"] += 1
        changed_report = compare_rust(parse_fixture(changed), actuals)
        self.assertEqual(len(changed_report.mismatches), 1)
        self.assertEqual(
            changed_report.mismatches[0].vector_id,
            f"channel/{changed_case['name']}",
        )
        self.assertEqual(
            changed_report.mismatches[0].expected,
            changed_case["expected"],
        )
        self.assertEqual(
            changed_report.mismatches[0].actual,
            changed_case["expected"] - 1,
        )

    def test_rust_container_matches_independent_python_packer(self):
        actuals = evaluate_rust(self.fixture)
        case = next(
            case for case in self.fixture.cases
            if case.category is Category.FIRMWARE_CONTAINER_CHECKSUM
        )
        raw = bytes(range(256)) * 48
        packed = pack_payload(raw, case.input["author"], case.input["version"])
        actual = actuals[case.vector_id]

        self.assertEqual(actual["raw_sha256"], hashlib.sha256(raw).hexdigest())
        self.assertEqual(actual["packed_length"], len(packed))
        self.assertEqual(
            actual["packed_sha256"],
            hashlib.sha256(packed).hexdigest(),
        )
        self.assertEqual(actual["appended_crc_bytes"], packed[-2:].hex())

    def test_rust_bridge_rejects_missing_channel_result(self):
        omitted_id = "channel/first_memory_valid"
        output = "\n".join(
            "\t".join(
                (
                    case.vector_id,
                    "00\t0000"
                    if case.category is Category.FIRMWARE_CONTAINER_CHECKSUM
                    else "0x0000=0000000000000000"
                    if case.category is Category.SETTINGS
                    else "0",
                )
            )
            for case in self.fixture.cases
            if case.vector_id != omitted_id
        )
        completed = SimpleNamespace(returncode=0, stdout=output, stderr="")

        with patch("compare_rust.subprocess.run", return_value=completed):
            with self.assertRaisesRegex(
                RuntimeError, f"Rust bridge omitted supported vectors: {omitted_id}"
            ):
                evaluate_rust(self.fixture)

    def test_invalid_scalar_input_is_rejected(self):
        data = json.loads((ROOT / "tests/reference-vectors/golden.json").read_text())
        data["cases"][0]["input"] = "channel=0,,band=0"
        with self.assertRaisesRegex(VectorFormatError, "invalid assignment"):
            parse_fixture(data)

    def test_scalar_assignments_must_match_domain_and_operation(self):
        data = json.loads((ROOT / "tests/reference-vectors/golden.json").read_text())
        data["cases"][0]["input"] = "tenths_hz=oops"
        with self.assertRaisesRegex(VectorFormatError, "wrong channel operation"):
            parse_fixture(data)

    def test_nested_numeric_type_mismatch_is_not_a_match(self):
        checksum_case = next(
            case for case in self.fixture.cases
            if case.category is Category.FIRMWARE_CONTAINER_CHECKSUM
        )
        report = run_vectors(
            self.fixture,
            {
                Category.FIRMWARE_CONTAINER_CHECKSUM: lambda case: {
                    **case.expected,
                    "packed_length": float(case.expected["packed_length"]),
                }
            },
        )
        self.assertEqual(len(report.mismatches), 1)
        self.assertEqual(report.mismatches[0].vector_id, checksum_case.vector_id)
        self.assertFalse(report.complete)

    def test_mismatch_values_are_snapshotted_per_vector(self):
        shared_result = {}

        def evaluate(case):
            shared_result.clear()
            shared_result["returned_for"] = case.name
            return shared_result

        report = run_vectors(self.fixture, {Category.SETTINGS: evaluate})
        actual_by_id = {mismatch.vector_id: mismatch.actual for mismatch in report.mismatches}
        self.assertEqual(actual_by_id["settings/standard"], {"returned_for": "standard"})
        self.assertEqual(actual_by_id["settings/boundary"], {"returned_for": "boundary"})

    def test_evaluator_cannot_mutate_fixture_expectations(self):
        settings_cases = [
            case for case in self.fixture.cases if case.category is Category.SETTINGS
        ]
        expected_by_id = {
            case.vector_id: deepcopy(case.expected) for case in settings_cases
        }

        def mutate_expected(case):
            address = next(iter(case.expected))
            case.expected[address] = "mutated"
            return case.expected

        mutated_report = run_vectors(
            self.fixture, {Category.SETTINGS: mutate_expected}
        )
        self.assertEqual(len(mutated_report.mismatches), len(settings_cases))
        self.assertEqual(
            {mismatch.vector_id: mismatch.expected for mismatch in mutated_report.mismatches},
            expected_by_id,
        )
        self.assertEqual(
            {case.vector_id: case.expected for case in settings_cases},
            expected_by_id,
        )

        subsequent_report = run_vectors(
            self.fixture, {Category.SETTINGS: lambda case: case.expected}
        )
        self.assertEqual(subsequent_report.matched, len(settings_cases))
        self.assertEqual(subsequent_report.mismatches, ())

    def test_mismatch_with_mixed_type_keys_formats(self):
        report = run_vectors(
            self.fixture,
            {
                Category.SETTINGS: lambda case: {
                    **case.expected,
                    123: "unexpected",
                }
            },
        )

        diagnostic = format_report(report)
        self.assertIn("Mismatch settings/standard", diagnostic)
        self.assertIn('"123":"unexpected"', diagnostic)

    def test_invalid_settings_write_is_rejected(self):
        data = json.loads((ROOT / "tests/reference-vectors/golden.json").read_text())
        settings_case = next(case for case in data["cases"] if case["category"] == "settings")
        address = next(iter(settings_case["expected"]))
        settings_case["expected"][address] = "0102"
        with self.assertRaisesRegex(VectorFormatError, "exactly eight bytes"):
            parse_fixture(data)

    def test_seeded_mismatch_reports_vector_id_expected_and_actual(self):
        report = run_vectors(
            self.fixture,
            {Category.CHANNEL: lambda _case: 2},
        )
        diagnostic = format_report(report)
        self.assertIn("Mismatch channel/first_memory_valid", diagnostic)
        self.assertIn("expected 1", diagnostic)
        self.assertIn("actual 2", diagnostic)
        self.assertIn("Unported domains (not executed)", diagnostic)

    def test_unported_domains_are_not_reported_as_coverage(self):
        report = run_vectors(self.fixture, {})
        diagnostic = format_report(report)
        self.assertIn("0/39 matched (0 executed)", diagnostic)
        self.assertIn("channel (12 cases)", diagnostic)
        self.assertIn("settings (2 cases)", diagnostic)
        self.assertIn("channel: channel/first_memory_valid", diagnostic)
        self.assertIn("No vector cases were executed; this is not parity coverage.", diagnostic)
        self.assertFalse(report.complete)


if __name__ == "__main__":
    unittest.main()
