import os
from pathlib import Path
import subprocess
import sys
import unittest


ROOT = Path(__file__).resolve().parents[2]


class ReferenceVectorCaptureTest(unittest.TestCase):
    def test_pinned_c_capture_matches_golden_values(self):
        self.assertTrue(
            os.environ.get("UV_K5_C_REFERENCE"),
            "set UV_K5_C_REFERENCE to the clean pinned C checkout",
        )
        result = subprocess.run(
            [sys.executable, ROOT / "tools" / "reference-vectors" / "capture.py", "--check"],
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
