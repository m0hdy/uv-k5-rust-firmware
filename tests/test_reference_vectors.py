import json
from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools" / "reference-vectors"))

from runner import (  # noqa: E402
    Category,
    VectorFormatError,
    format_report,
    load_fixture,
    parse_fixture,
    run_vectors,
)


class ReferenceVectorRunnerTests(unittest.TestCase):
    def setUp(self):
        self.fixture = load_fixture()

    def test_checked_in_fixture_loads_without_c_reference(self):
        self.assertEqual(len(self.fixture.cases), 39)
        self.assertEqual(
            self.fixture.reference_commit,
            "7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3",
        )

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
        self.assertIn("No vector cases were executed; this is not parity coverage.", diagnostic)
        self.assertFalse(report.complete)


if __name__ == "__main__":
    unittest.main()
