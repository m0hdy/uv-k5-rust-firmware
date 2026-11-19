#!/usr/bin/env python3
"""Verify ARM GNU binutils can be provisioned from the signed Ubuntu archive."""

import os
from pathlib import Path
import subprocess
import tempfile

from provision_arm_binutils import (
    DEFAULT_PACKAGE_VERSION,
    install_from_apt,
    system_codename,
    toolchain_available,
    write_direct_archive_sources,
)


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="arm-binutils-archive-smoke-") as directory:
        root = Path(directory)
        cache_dir = root / "debs"
        cache_dir.mkdir()
        sources = root / "ubuntu.sources.list"
        write_direct_archive_sources(sources, system_codename())
        try:
            install_from_apt(cache_dir, DEFAULT_PACKAGE_VERSION, sources)
        finally:
            subprocess.run(
                [
                    "sudo",
                    "chown",
                    "-R",
                    f"{os.getuid()}:{os.getgid()}",
                    str(cache_dir),
                ],
                check=True,
            )

    if not toolchain_available():
        raise RuntimeError("direct Ubuntu archive provisioning did not produce the supported toolchain")
    print("Signed direct Ubuntu archive provisioning succeeded.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
