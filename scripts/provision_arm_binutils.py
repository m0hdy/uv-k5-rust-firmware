#!/usr/bin/env python3
"""Install or restore the pinned ARM GNU binutils used by CI."""

from __future__ import annotations

import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import time
from collections.abc import Callable


PACKAGE_NAME = "binutils-arm-none-eabi"
DEFAULT_PACKAGE_VERSION = "2.42-1ubuntu1+23"
REQUIRED_TOOLS = ("as", "ld", "objcopy", "objdump", "nm")
MINIMUM_BINUTILS_VERSION = (2, 42)
APT_ATTEMPTS = 2
APT_DELAY_SECONDS = 5


def toolchain_available(
    which: Callable[[str], str | None] = shutil.which,
    runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run,
) -> bool:
    paths = [which(f"arm-none-eabi-{tool}") for tool in REQUIRED_TOOLS]
    if any(path is None for path in paths):
        return False

    for path in paths:
        assert path is not None
        result = runner(
            [path, "--version"],
            check=True,
            capture_output=True,
            text=True,
        )
        output = re.sub(r"\([^)]*\)", " ", result.stdout + result.stderr)
        matches = re.findall(r"\b(\d+)\.(\d+)(?:\.\d+)?\b", output)
        if not matches:
            return False
        version = tuple(map(int, matches[-1]))
        if version < MINIMUM_BINUTILS_VERSION:
            return False
    return True


def retry(
    operation: Callable[[], None],
    *,
    description: str,
    attempts: int = APT_ATTEMPTS,
    sleep: Callable[[float], None] = time.sleep,
) -> None:
    for attempt in range(1, attempts + 1):
        try:
            operation()
            return
        except subprocess.CalledProcessError as error:
            if attempt == attempts:
                raise
            print(
                f"{description} failed (attempt {attempt}/{attempts}): {error}; retrying",
                file=sys.stderr,
            )
            sleep(APT_DELAY_SECONDS * attempt)


def install_cached_packages(
    cache_dir: Path,
    runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run,
) -> bool:
    archives = sorted(cache_dir.glob("*.deb"))
    if not archives:
        return False

    try:
        packages_to_unpack = []
        for archive in archives:
            package = runner(
                ["dpkg-deb", "--field", str(archive), "Package"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            cached_version = runner(
                ["dpkg-deb", "--field", str(archive), "Version"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            installed = runner(
                ["dpkg-query", "-W", "-f=${db:Status-Status}", package],
                check=False,
                capture_output=True,
                text=True,
            )
            if installed.returncode == 0 and installed.stdout.strip() == "installed":
                installed_version = runner(
                    ["dpkg-query", "-W", "-f=${Version}", package],
                    check=True,
                    capture_output=True,
                    text=True,
                ).stdout.strip()
                comparison = runner(
                    [
                        "dpkg",
                        "--compare-versions",
                        installed_version,
                        "ge",
                        cached_version,
                    ],
                    check=False,
                )
                if comparison.returncode == 0:
                    continue
            packages_to_unpack.append(str(archive))

        if packages_to_unpack:
            runner(["sudo", "dpkg", "--unpack", *packages_to_unpack], check=True)
            runner(["sudo", "dpkg", "--configure", "-a"], check=True)
    except subprocess.CalledProcessError as error:
        print(f"Cached ARM binutils packages could not be installed: {error}", file=sys.stderr)
        return False
    return True


def write_direct_archive_sources(path: Path, codename: str) -> None:
    suites = (codename, f"{codename}-updates", f"{codename}-backports", f"{codename}-security")
    path.write_text(
        "".join(
            "deb [signed-by=/usr/share/keyrings/ubuntu-archive-keyring.gpg] "
            f"https://archive.ubuntu.com/ubuntu/ {suite} main universe\n"
            for suite in suites
        ),
        encoding="utf-8",
    )


def apt_options(cache_dir: Path, source_file: Path | None) -> list[str]:
    options = [
        "-o",
        "Acquire::http::Timeout=20",
        "-o",
        "Acquire::https::Timeout=20",
        "-o",
        "Acquire::Retries=1",
        "-o",
        f"Dir::Cache::archives={cache_dir}/",
    ]
    if source_file is not None:
        options.extend(
            [
                "-o",
                f"Dir::Etc::sourcelist={source_file}",
                "-o",
                "Dir::Etc::sourceparts=-",
                "-o",
                "APT::Get::List-Cleanup=0",
            ]
        )
    return options


def install_from_apt(
    cache_dir: Path,
    package_version: str,
    source_file: Path | None,
    runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run,
) -> None:
    base = ["sudo", "apt-get", *apt_options(cache_dir, source_file)]
    package = f"{PACKAGE_NAME}={package_version}"
    runner([*base, "update"], check=True)
    runner(
        [
            *base,
            "install",
            "--yes",
            "--download-only",
            "--reinstall",
            "--no-install-recommends",
            package,
        ],
        check=True,
    )
    runner(
        [
            *base,
            "install",
            "--yes",
            "--no-install-recommends",
            "--no-download",
            package,
        ],
        check=True,
    )


def install_with_fallback(
    primary: Callable[[], None],
    fallback: Callable[[], None],
    *,
    sleep: Callable[[float], None] = time.sleep,
) -> str:
    primary_error = ""
    try:
        retry(primary, description="Ubuntu runner package sources", sleep=sleep)
        return "runner-sources"
    except subprocess.CalledProcessError as error:
        primary_error = str(error)
        print(
            f"Ubuntu runner package sources failed after bounded retries: {error}; "
            "trying the direct Ubuntu archive",
            file=sys.stderr,
        )

    try:
        retry(fallback, description="Direct Ubuntu archive", sleep=sleep)
    except subprocess.CalledProcessError as fallback_error:
        raise RuntimeError(
            "ARM GNU binutils installation failed from both the runner package sources "
            f"({primary_error}) and direct Ubuntu archive ({fallback_error})"
        ) from fallback_error
    return "direct-archive"


def system_codename() -> str:
    release = platform.freedesktop_os_release()
    codename = release.get("UBUNTU_CODENAME") or release.get("VERSION_CODENAME")
    if not codename:
        raise RuntimeError("could not determine the Ubuntu release codename for the fallback source")
    return codename


def write_cache_output(cache_populated: bool) -> None:
    output = os.environ.get("GITHUB_OUTPUT")
    if output:
        with open(output, "a", encoding="utf-8") as output_file:
            output_file.write(f"cache-populated={str(cache_populated).lower()}\n")


def provision(
    cache_dir: Path,
    package_version: str,
    *,
    runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run,
    which: Callable[[str], str | None] = shutil.which,
    sleep: Callable[[float], None] = time.sleep,
) -> bool:
    cache_dir.mkdir(parents=True, exist_ok=True)
    if toolchain_available(which, runner):
        print("ARM GNU binutils are already available at a supported version.")
        return False

    if install_cached_packages(cache_dir, runner) and toolchain_available(which, runner):
        print("Installed ARM GNU binutils from the Actions cache.")
        return False

    primary = lambda: install_from_apt(cache_dir, package_version, None, runner)
    with tempfile.TemporaryDirectory(prefix="arm-binutils-sources-") as directory:
        source_file = Path(directory) / "ubuntu.sources.list"
        write_direct_archive_sources(source_file, system_codename())
        fallback = lambda: install_from_apt(cache_dir, package_version, source_file, runner)
        install_with_fallback(primary, fallback, sleep=sleep)

    if not toolchain_available(which, runner):
        raise RuntimeError(
            "installation completed but required ARM GNU binutils "
            f"{REQUIRED_TOOLS} at version {MINIMUM_BINUTILS_VERSION[0]}.{MINIMUM_BINUTILS_VERSION[1]} "
            "are still unavailable"
        )

    cached_packages = sorted(cache_dir.glob("*.deb"))
    if cached_packages:
        runner(
            [
                "sudo",
                "chown",
                "-R",
                f"{os.getuid()}:{os.getgid()}",
                str(cache_dir),
            ],
            check=True,
        )
    return bool(cached_packages)


def main() -> int:
    cache_dir = Path(os.environ["ARM_BINUTILS_CACHE_DIR"])
    package_version = os.environ.get("ARM_BINUTILS_VERSION", DEFAULT_PACKAGE_VERSION)
    try:
        cache_populated = provision(cache_dir, package_version)
    except (OSError, subprocess.CalledProcessError, RuntimeError) as error:
        print(f"ARM GNU binutils provisioning failed: {error}", file=sys.stderr)
        return 1
    write_cache_output(cache_populated)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
