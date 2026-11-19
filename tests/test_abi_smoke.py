import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUNNER = ROOT / "abi" / "smoke" / "run.py"


class AbiSmokeTests(unittest.TestCase):
    def test_host_call_and_thumb_target_link(self):
        with tempfile.TemporaryDirectory() as output_dir:
            result = subprocess.run(
                [
                    sys.executable,
                    str(RUNNER),
                    "--output-dir",
                    output_dir,
                ],
                cwd=ROOT,
                text=True,
                capture_output=True,
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
