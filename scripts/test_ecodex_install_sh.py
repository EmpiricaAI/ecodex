from __future__ import annotations

import io
import os
import subprocess
import tarfile
from pathlib import Path

INSTALL_SCRIPT = Path(__file__).with_name("install.sh")
BINARIES = ("ecodex", "codex-empirica-plugin", "codex-empirica-translator", "codex-code-mode-host")


def _archive(path: Path) -> None:
    with tarfile.open(path, "w:gz") as archive:
        for name in BINARIES:
            data = b"#!/bin/sh\nexit 0\n"
            info = tarfile.TarInfo(name)
            info.mode = 0o755
            info.size = len(data)
            archive.addfile(info, io.BytesIO(data))


def test_missing_checksum_cannot_install_unverified_archive(tmp_path: Path) -> None:
    archive = tmp_path / "fixture.tar.gz"
    _archive(archive)
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    curl = fake_bin / "curl"
    curl.write_text(
        "#!/bin/sh\n"
        "out=''\n"
        "previous=''\n"
        'for argument in "$@"; do\n'
        "  if [ \"$previous\" = '-o' ]; then out=$argument; fi\n"
        "  previous=$argument\n"
        "done\n"
        'case "$*" in\n'
        "  *.sha256*) exit 22 ;;\n"
        '  *.tar.gz*) cp "$FAKE_ARCHIVE" "$out"; exit 0 ;;\n'
        "  *) exit 22 ;;\n"
        "esac\n",
        encoding="utf-8",
    )
    curl.chmod(0o755)
    install_dir = tmp_path / "install"

    result = subprocess.run(
        [str(INSTALL_SCRIPT), "--prefix", str(install_dir)],
        text=True,
        capture_output=True,
        env={
            **os.environ,
            "PATH": f"{fake_bin}:{os.environ['PATH']}",
            "FAKE_ARCHIVE": str(archive),
            "ECODEX_VERSION": "v1.2.3",
        },
    )

    assert result.returncode != 0
    assert "checksum download failed" in result.stderr
    assert not install_dir.exists()
