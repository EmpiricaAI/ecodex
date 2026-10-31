from __future__ import annotations

import os
import shlex
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


GREEN_NIGHTLY = "41\tcompleted\thttps://example.invalid/runs/41\tsuccess"
VERIFY_FLAGS = ("--patch", "--skip-commit", "--skip-tag", "--skip-changelog", "--verify-install")


def _fake_gh(root: Path, row: str, returncode: int) -> Path:
    """A gh that records its arguments and prints one `gh run list` row."""
    bin_dir = root.parent / "bin"
    bin_dir.mkdir(exist_ok=True)
    printed = f"printf '%s\\n' {shlex.quote(row)}\n" if row else ""
    _write_executable(
        bin_dir / "gh",
        "#!/usr/bin/env bash\n"
        f"printf '%s\\n' \"$*\" >> {shlex.quote(str(bin_dir / 'gh-calls'))}\n"
        f"{printed}exit {returncode}\n",
    )
    return bin_dir


def _run_release(
    root: Path,
    script: Path,
    *flags: str,
    gh_row: str = GREEN_NIGHTLY,
    gh_returncode: int = 0,
) -> subprocess.CompletedProcess[str]:
    bin_dir = _fake_gh(root, gh_row, gh_returncode)
    return subprocess.run(
        [str(script), *(flags or VERIFY_FLAGS)],
        cwd=root,
        text=True,
        capture_output=True,
        env={
            **os.environ,
            "GIT_CONFIG_NOSYSTEM": "1",
            "PATH": f"{bin_dir}{os.pathsep}{os.environ['PATH']}",
        },
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


GATE_FLAGS = ("--patch", "--skip-commit", "--skip-tag", "--skip-changelog")


def _head(root: Path) -> str:
    return subprocess.run(
        ["git", "-C", str(root), "rev-parse", "HEAD"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def _version(root: Path) -> str:
    return (root / "codex-rs" / "Cargo.toml").read_text(encoding="utf-8")


def _gh_calls(root: Path) -> str:
    calls = root.parent / "bin" / "gh-calls"
    return calls.read_text(encoding="utf-8") if calls.is_file() else ""


def test_a_commit_the_nightly_never_ran_on_is_not_released(tmp_path: Path) -> None:
    root, script = _release_fixture(tmp_path, "#!/usr/bin/env bash\nexit 0\n")

    result = _run_release(root, script, *GATE_FLAGS, gh_row="")

    assert result.returncode != 0
    assert f"nightly.yml has not run on {_head(root)}" in result.stderr
    assert "gh workflow run nightly.yml -R EmpiricaAI/ecodex --ref" in result.stderr
    assert 'version = "1.2.3"' in _version(root), "the gate stops before the bump"
    calls = _gh_calls(root)
    assert f"--commit {_head(root)}" in calls, "the full SHA, never a short one"
    assert "-R EmpiricaAI/ecodex" in calls
    assert "--workflow nightly.yml" in calls


def test_a_nightly_still_running_is_not_green(tmp_path: Path) -> None:
    root, script = _release_fixture(tmp_path, "#!/usr/bin/env bash\nexit 0\n")

    result = _run_release(
        root, script, *GATE_FLAGS, gh_row="7\tin_progress\thttps://example.invalid/runs/7\t"
    )

    assert result.returncode != 0
    assert "is in_progress: https://example.invalid/runs/7" in result.stderr
    assert "gh run watch -R EmpiricaAI/ecodex 7" in result.stderr
    assert 'version = "1.2.3"' in _version(root)


def test_a_failed_nightly_stops_the_release(tmp_path: Path) -> None:
    root, script = _release_fixture(tmp_path, "#!/usr/bin/env bash\nexit 0\n")

    result = _run_release(
        root, script, *GATE_FLAGS, gh_row="8\tcompleted\thttps://example.invalid/runs/8\tfailure"
    )

    assert result.returncode != 0
    assert "ended failure: https://example.invalid/runs/8" in result.stderr
    assert 'version = "1.2.3"' in _version(root)


def test_an_unreadable_run_list_fails_closed(tmp_path: Path) -> None:
    root, script = _release_fixture(tmp_path, "#!/usr/bin/env bash\nexit 0\n")

    result = _run_release(root, script, *GATE_FLAGS, gh_row="", gh_returncode=4)

    assert result.returncode != 0
    assert "couldn't read nightly.yml runs" in result.stderr
    assert 'version = "1.2.3"' in _version(root)


def test_a_green_nightly_lets_the_dry_run_through(tmp_path: Path) -> None:
    root, script = _release_fixture(tmp_path, "#!/usr/bin/env bash\nexit 0\n")

    result = _run_release(root, script, "--dry-run", *GATE_FLAGS)

    assert result.returncode == 0, result.stderr
    assert f"Nightly passed on {_head(root)[:10]}" in result.stderr


def test_the_post_cut_recovery_path_does_not_ask_for_a_nightly(tmp_path: Path) -> None:
    root, script = _release_fixture(tmp_path, "#!/usr/bin/env bash\nexit 0\n")

    result = _run_release(
        root,
        script,
        "--explicit",
        "1.2.3",
        "--force-version",
        "--dry-run",
        "--skip-commit",
        "--skip-tag",
        "--skip-changelog",
        gh_row="",
    )

    assert result.returncode == 0, result.stderr
    assert "--force-version: not checking the nightly" in result.stderr
    assert _gh_calls(root) == ""
