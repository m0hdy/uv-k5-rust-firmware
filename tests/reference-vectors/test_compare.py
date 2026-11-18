import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest


ROOT = Path(__file__).resolve().parents[2]
COMPARATOR_DIR = ROOT / "tools" / "reference-vectors"
sys.path.insert(0, str(COMPARATOR_DIR))
import compare


class ReferenceVectorComparisonTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        fixture_path = ROOT / "tests" / "reference-vectors" / "golden.json"
        cls.cases = json.loads(fixture_path.read_text(encoding="utf-8"))["cases"]

    def test_identical_canonical_inputs_and_results_match(self):
        self.assertEqual(compare.compare_cases(self.cases, copy.deepcopy(self.cases)), [])

    def test_input_mismatch_is_reported_for_its_case(self):
        captured = copy.deepcopy(self.cases)
        captured[0]["input"] = "different input"

        mismatches = compare.compare_cases(self.cases, captured)

        self.assertEqual(mismatches, ["channel/first_memory_valid: input mismatch"])

    def test_wrong_result_is_reported_for_its_case(self):
        captured = copy.deepcopy(self.cases)
        captured[0]["expected"] = 999

        mismatches = compare.compare_cases(self.cases, captured)

        self.assertEqual(
            mismatches,
            [
                "channel/first_memory_valid: result mismatch "
                "(canonical 1, captured 999)"
            ],
        )

    def test_wrong_reference_revision_is_rejected(self):
        environment = os.environ.copy()
        environment["UV_K5_C_REFERENCE"] = str(ROOT)
        result = subprocess.run(
            [sys.executable, COMPARATOR_DIR / "compare.py"],
            cwd=ROOT,
            env=environment,
            text=True,
            capture_output=True,
            check=False,
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("C reference must be at", result.stderr)


if __name__ == "__main__":
    unittest.main()
