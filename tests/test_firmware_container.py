import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from firmware_container import (  # noqa: E402
    ContainerError,
    VERSION_OFFSET,
    decode_container,
    pack_payload,
)


class FirmwareContainerTests(unittest.TestCase):
    def test_approved_reference_fixture_matches_byte_for_byte(self):
        fixture_path = ROOT / "tests" / "reference-vectors" / "golden.json"
        fixture = json.loads(fixture_path.read_text(encoding="utf-8"))
        case = next(
            item for item in fixture["cases"]
            if item["category"] == "firmware_container_checksum"
        )
        raw = bytes(range(256)) * 48
        packed = pack_payload(raw, case["input"]["author"], case["input"]["version"])
        expected = case["expected"]

        self.assertEqual(hashlib.sha256(raw).hexdigest(), expected["raw_sha256"])
        self.assertEqual(len(packed), expected["packed_length"])
        self.assertEqual(hashlib.sha256(packed).hexdigest(), expected["packed_sha256"])
        self.assertEqual(packed[-2:].hex(), expected["appended_crc_bytes"])
        self.assertEqual(decode_container(packed).payload, raw)

    def test_short_payloads_append_version_without_padding(self):
        for length in (0, 1, VERSION_OFFSET - 1):
            with self.subTest(length=length):
                raw = bytes(index & 0xFF for index in range(length))
                packed = pack_payload(raw, "Short", "1")
                contents = decode_container(packed, "Short", "1")
                self.assertEqual(contents.version_offset, length)
                self.assertEqual(contents.payload, raw)
                self.assertEqual(len(packed), length + 18)

    def test_payload_at_and_above_insertion_offset_round_trips(self):
        for length in (VERSION_OFFSET, VERSION_OFFSET + 1, 16384):
            with self.subTest(length=length):
                raw = bytes(index & 0xFF for index in range(length))
                contents = decode_container(pack_payload(raw, "Test", "2.0"))
                self.assertEqual(contents.version_offset, VERSION_OFFSET)
                self.assertEqual(contents.payload, raw)

    def test_crc_corruption_and_truncation_are_rejected(self):
        packed = pack_payload(b"synthetic firmware", "Test", "2.0")
        corrupted = bytearray(packed)
        corrupted[0] ^= 0x01
        with self.assertRaisesRegex(ContainerError, "CRC-XMODEM mismatch"):
            decode_container(bytes(corrupted))
        with self.assertRaisesRegex(ContainerError, "too short"):
            decode_container(packed[:17])

    def test_version_field_and_ascii_requirements(self):
        packed = pack_payload(b"payload", "Author", "v1")
        self.assertEqual(decode_container(packed).version_field, b"*Author v1\0" + bytes(5))
        with self.assertRaisesRegex(ContainerError, "does not match"):
            decode_container(packed, "Other", "v1")
        with self.assertRaisesRegex(ContainerError, "ASCII"):
            pack_payload(b"payload", "Au\u00f6thor", "v1")

    def test_cli_pack_validate_and_unpack(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            raw_path = directory / "raw.bin"
            packed_path = directory / "packed.bin"
            recovered_path = directory / "recovered.bin"
            raw = b"host-side synthetic payload"
            raw_path.write_bytes(raw)

            command = [sys.executable, str(ROOT / "scripts" / "firmware_container.py")]
            subprocess.run(
                command + ["pack", str(raw_path), str(packed_path), "--author", "CLI", "--version", "1"],
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                command + ["validate", str(packed_path), "--author", "CLI", "--version", "1"],
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                command + ["unpack", str(packed_path), str(recovered_path)],
                check=True,
                capture_output=True,
                text=True,
            )
            self.assertEqual(recovered_path.read_bytes(), raw)


if __name__ == "__main__":
    unittest.main()
