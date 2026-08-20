from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

RELEASE_SCRIPT = Path(__file__).with_name("release.sh")


def _write_executable(path: Path, text: str) -> None:
    path.write_text(text, encoding="utf-8")
    path.chmod(0o755)


def _release_fixture(tmp_path: Path, installer: str) -> tuple[Path, Path]:
    root = tmp_path / "repo"
    scripts = root / "scripts"
    codex_rs = root / "codex-rs"
    scripts.mkdir(parents=True)
    codex_rs.mkdir()
    shutil.copy2(RELEASE_SCRIPT, scripts / "release.sh")
    _write_executable(scripts / "install.sh", installer)
    (codex_rs / "Cargo.toml").write_text(
        '[workspace.package]\nversion = "1.2.3"\n', encoding="utf-8"
    )
    (root / "CHANGELOG.md").write_text(
        "# Changelog\n\n## [Unreleased]\n\n- audit fixture\n",
        encoding="utf-8",
    )
    subprocess.run(["git", "init", str(root)], check=True, capture_output=True)
    subprocess.run(["git", "-C", str(root), "add", "."], check=True)
    git_env = {
        **os.environ,
        "GIT_AUTHOR_NAME": "Verdict Audit",
        "GIT_AUTHOR_EMAIL": "audit@example.invalid",
        "GIT_COMMITTER_NAME": "Verdict Audit",
        "GIT_COMMITTER_EMAIL": "audit@example.invalid",
    }
    subprocess.run(
        ["git", "-C", str(root), "commit", "-m", "fixture"],
        check=True,
        capture_output=True,
        env=git_env,
    )
    return root, scripts / "release.sh"


def _run_release(root: Path, script: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            str(script),
            "--patch",
            "--skip-commit",
            "--skip-tag",
            "--skip-changelog",
            "--verify-install",
        ],
        cwd=root,
        text=True,
        capture_output=True,
        env={**os.environ, "GIT_CONFIG_NOSYSTEM": "1"},
    )


def test_requested_install_verification_cannot_skip(tmp_path: Path) -> None:
    root, script = _release_fixture(
        tmp_path,
        '#!/usr/bin/env bash\ntouch "$(dirname "$0")/verify-ran"\nexit 7\n',
    )

    result = _run_release(root, script)

    assert result.returncode != 0
    assert (root / "scripts" / "verify-ran").is_file()
    assert "install.sh failed against v1.2.4" in result.stderr


def test_version_substring_is_not_an_exact_version_verdict(tmp_path: Path) -> None:
    root, script = _release_fixture(
        tmp_path,
        "#!/usr/bin/env bash\n"
        "prefix=$2\n"
        'mkdir -p "$prefix"\n'
        "printf '%s\\n' '#!/usr/bin/env bash' 'echo ecodex 91.2.40' >\"$prefix/ecodex\"\n"
        'chmod +x "$prefix/ecodex"\n',
    )

    result = _run_release(root, script)

    assert result.returncode != 0
    assert "expected exact version token '1.2.4'" in result.stderr
