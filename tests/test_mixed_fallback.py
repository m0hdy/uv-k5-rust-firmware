import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUNNER = ROOT / "mixed-fallback" / "run.py"


@unittest.skipUnless(
    os.environ.get("UVK5_RUN_MIXED_FALLBACK") == "1",
    "set UVK5_RUN_MIXED_FALLBACK=1 to run the opt-in C/Rust image build",
)
class MixedFallbackTests(unittest.TestCase):
    def test_default_firmware_build_does_not_need_clang_or_reference_checkout(self):
        environment = os.environ.copy()
        environment["CLANG"] = "/missing/clang"
        environment.pop("UV_K5_C_REFERENCE", None)
        result = subprocess.run(
            [
                "cargo",
                "build",
                "--locked",
                "--target",
                "thumbv6m-none-eabi",
                "--release",
                "--bin",
                "uv-k5-rust-firmware",
            ],
            cwd=ROOT,
            env=environment,
            text=True,
            capture_output=True,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_host_comparison_and_both_target_selections(self):
        with tempfile.TemporaryDirectory() as output_dir:
            report = Path(output_dir) / "evidence.md"
            result = subprocess.run(
                [
                    sys.executable,
                    str(RUNNER),
                    "--output-dir",
                    output_dir,
                    "--report",
                    str(report),
                ],
                cwd=ROOT,
                text=True,
                capture_output=True,
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            evidence = report.read_text(encoding="utf-8")
            self.assertIn("C selected", evidence)
            self.assertIn("RUST selected", evidence)
            self.assertIn("2,048 bytes reserved for stack", evidence)


if __name__ == "__main__":
    unittest.main()
