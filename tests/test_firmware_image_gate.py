import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from check_firmware_image import (  # noqa: E402
    ImageValidationError,
    validate_image,
)
from firmware_container import pack_payload  # noqa: E402


class FirmwareImageGateTests(unittest.TestCase):
    def test_valid_image_reports_all_gates_and_matching_payloads(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = [
                Path(directory) / name
                for name in ("firmware.elf", "firmware.map", "raw.bin", "packed.bin")
            ]
            elf, map_path, raw, packed = paths
            elf.write_bytes(b"ELF")
            map_path.write_text("map", encoding="ascii")
            payload = b"development image"
            raw.write_bytes(payload)
            packed.write_bytes(pack_payload(payload, "UV-K5", "dev"))
            report = type(
                "BudgetReport",
                (),
                {"errors": [], "flash_load_bytes": 1},
            )()

            with (
                patch("check_firmware_image.validate") as startup_check,
                patch(
                    "check_firmware_image.analyze", return_value=report
                ) as memory_check,
                patch(
                    "check_firmware_image.format_report",
                    return_value="Budget check passed",
                ),
            ):
                output = validate_image(elf, map_path, raw, packed, "UV-K5", "dev")

            startup_check.assert_called_once_with(elf, map_path)
            memory_check.assert_called_once()
            self.assertIn("Startup layout valid", output)
            self.assertIn("Budget check passed", output)
            self.assertIn("Container valid", output)
            self.assertIn("Raw and packed payloads match", output)

    def test_raw_and_packed_mismatch_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            elf, map_path, raw, packed = (
                Path(directory) / name
                for name in ("firmware.elf", "firmware.map", "raw.bin", "packed.bin")
            )
            elf.write_bytes(b"ELF")
            map_path.write_text("map", encoding="ascii")
            raw.write_bytes(b"invalid image")
            packed.write_bytes(pack_payload(b"original image", "UV-K5", "dev"))
            report = type("BudgetReport", (), {"errors": []})()

            with (
                patch("check_firmware_image.validate"),
                patch("check_firmware_image.analyze", return_value=report),
                patch(
                    "check_firmware_image.format_report",
                    return_value="Budget check passed",
                ),
            ):
                with self.assertRaisesRegex(
                    ImageValidationError,
                    "packed container payload does not match the raw image",
                ):
                    validate_image(elf, map_path, raw, packed, "UV-K5", "dev")


if __name__ == "__main__":
    unittest.main()
