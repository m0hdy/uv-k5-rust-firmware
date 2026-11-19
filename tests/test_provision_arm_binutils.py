import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from provision_arm_binutils import (  # noqa: E402
    DEFAULT_PACKAGE_VERSION,
    install_cached_packages,
    install_from_apt,
    install_with_fallback,
    toolchain_available,
    write_direct_archive_sources,
)


class ProvisionArmBinutilsTests(unittest.TestCase):
    def test_supported_toolchain_requires_all_tools_and_minimum_version(self):
        available = lambda _name: "/usr/bin/arm-none-eabi-tool"
        runner = Mock(
            return_value=SimpleNamespace(stdout="GNU objcopy 2.42\n", stderr="")
        )
        self.assertTrue(toolchain_available(available, runner))

        old_runner = Mock(
            return_value=SimpleNamespace(stdout="GNU objcopy 2.41\n", stderr="")
        )
        self.assertFalse(toolchain_available(available, old_runner))
        self.assertFalse(toolchain_available(lambda name: None if name.endswith("-nm") else available(name), runner))

    def test_tool_versions_ignore_vendor_branding_and_check_every_tool(self):
        versions = {
            "as": "GNU assembler 2.42\n",
            "ld": "GNU ld 2.42\n",
            "objcopy": "GNU objcopy 2.42\n",
            "objdump": "GNU objdump 2.42\n",
            "nm": "GNU nm 2.42\n",
        }

        def runner(command, **kwargs):
            tool = command[0].rsplit("-", 1)[-1]
            return SimpleNamespace(stdout=versions[tool], stderr="")

        available = lambda name: f"/usr/bin/{name}"
        versions["objcopy"] = (
            "GNU objcopy (Arm GNU Toolchain 12.2.Rel1) 2.39.0.20221210\n"
        )
        self.assertFalse(toolchain_available(available, runner))

        versions["objcopy"] = "GNU objcopy 2.42\n"
        versions["ld"] = "GNU ld (GNU Binutils) 2.41\n"
        self.assertFalse(toolchain_available(available, runner))

    def test_cache_hit_installs_archives_without_apt(self):
        commands = []

        def runner(command, **kwargs):
            commands.append(command)
            if command[0] == "dpkg-deb":
                value = (
                    "binutils-arm-none-eabi\n"
                    if command[3] == "Package"
                    else DEFAULT_PACKAGE_VERSION
                )
                return SimpleNamespace(returncode=0, stdout=value)
            if command[0] == "dpkg-query":
                return SimpleNamespace(returncode=1, stdout="")
            if command[0] == "sudo":
                return SimpleNamespace(returncode=0, stdout="")
            raise AssertionError(f"unexpected command: {command}")

        with tempfile.TemporaryDirectory() as directory:
            cache_dir = Path(directory)
            (cache_dir / "binutils-arm-none-eabi.deb").touch()
            self.assertTrue(install_cached_packages(cache_dir, runner))

        self.assertEqual(
            [command for command in commands if command[0] == "sudo"],
            [
                ["sudo", "dpkg", "--unpack", str(cache_dir / "binutils-arm-none-eabi.deb")],
                ["sudo", "dpkg", "--configure", "-a"],
            ],
        )
        self.assertFalse(any(command[0] == "apt-get" for command in commands))

    def test_cache_hit_does_not_downgrade_newer_installed_packages(self):
        commands = []

        def runner(command, **kwargs):
            commands.append(command)
            if command[0] == "dpkg-deb":
                value = (
                    "binutils-common\n"
                    if command[3] == "Package"
                    else DEFAULT_PACKAGE_VERSION
                )
                return SimpleNamespace(returncode=0, stdout=value)
            if command[0] == "dpkg-query":
                if command[2] == "-f=${db:Status-Status}":
                    return SimpleNamespace(returncode=0, stdout="installed\n")
                return SimpleNamespace(returncode=0, stdout="2.42-1ubuntu1+24\n")
            if command[:2] == ["dpkg", "--compare-versions"]:
                self.assertEqual(
                    command[2:],
                    ["2.42-1ubuntu1+24", "ge", DEFAULT_PACKAGE_VERSION],
                )
                return SimpleNamespace(returncode=0)
            return SimpleNamespace(returncode=0)

        with tempfile.TemporaryDirectory() as directory:
            cache_dir = Path(directory)
            (cache_dir / "binutils-common.deb").touch()
            self.assertTrue(install_cached_packages(cache_dir, runner))

        self.assertFalse(any(command[:3] == ["sudo", "dpkg", "--unpack"] for command in commands))

    def test_cache_miss_downloads_pinned_packages_then_installs_offline(self):
        runner = Mock()
        with tempfile.TemporaryDirectory() as directory:
            install_from_apt(Path(directory), DEFAULT_PACKAGE_VERSION, None, runner)

        commands = [call.args[0] for call in runner.call_args_list]
        self.assertEqual(commands[0][-1], "update")
        self.assertIn("--download-only", commands[1])
        self.assertIn(f"binutils-arm-none-eabi={DEFAULT_PACKAGE_VERSION}", commands[1])
        self.assertIn("--no-download", commands[2])

    def test_retries_primary_source_then_uses_direct_archive(self):
        primary = Mock(side_effect=subprocess.CalledProcessError(1, "apt"))
        fallback = Mock()
        sleeps = []

        source = install_with_fallback(primary, fallback, sleep=sleeps.append)

        self.assertEqual(source, "direct-archive")
        self.assertEqual(primary.call_count, 2)
        fallback.assert_called_once()
        self.assertEqual(sleeps, [5])

    def test_terminal_failure_reports_both_sources(self):
        primary = Mock(side_effect=subprocess.CalledProcessError(1, "runner apt"))
        fallback = Mock(side_effect=subprocess.CalledProcessError(1, "direct apt"))

        with self.assertRaisesRegex(RuntimeError, "both the runner package sources.*direct Ubuntu archive"):
            install_with_fallback(primary, fallback, sleep=lambda _delay: None)

        self.assertEqual(primary.call_count, 2)
        self.assertEqual(fallback.call_count, 2)

    def test_direct_archive_fallback_uses_official_https_source(self):
        with tempfile.TemporaryDirectory() as directory:
            sources = Path(directory) / "sources.list"
            write_direct_archive_sources(sources, "noble")
            content = sources.read_text(encoding="utf-8")

        self.assertIn("https://archive.ubuntu.com/ubuntu/", content)
        self.assertIn("noble-security", content)
        self.assertNotIn("mirror+file:", content)


if __name__ == "__main__":
    unittest.main()
