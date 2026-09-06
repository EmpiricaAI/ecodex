"""Self-test for the UPSTREAM_SYNC_TAG drift-guard.

Each case has a real failure path — the stale-tag scenario (the 0.149-merge
near-miss) goes red, the patch-release scenario stays green — so the guard
cannot silently pass on a mismatch.
"""

from __future__ import annotations

import importlib.util
from pathlib import Path

SCRIPT = Path(__file__).with_name("check_upstream_sync_tag.py")
_spec = importlib.util.spec_from_file_location("check_upstream_sync_tag", SCRIPT)
assert _spec is not None and _spec.loader is not None
guard = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(guard)

_CARGO = """[workspace]
members = ["a"]

[workspace.package]
version = "0.152.0"
edition = "2024"

[workspace.dependencies]
gix = { version = "0.83.0" }
"""


def _write(tmp_path: Path, cargo: str, tag: str) -> tuple[Path, Path]:
    c = tmp_path / "Cargo.toml"
    t = tmp_path / "UPSTREAM_SYNC_TAG"
    c.write_text(cargo, encoding="utf-8")
    t.write_text(tag, encoding="utf-8")
    return c, t


def test_matching_minor_passes(tmp_path):
    c, t = _write(tmp_path, _CARGO, "rust-v0.152.0\n")
    assert guard.check(c, t) == []


def test_patch_release_passes(tmp_path):
    # 0.152.1 workspace stays on the rust-v0.152.0 upstream base — same minor.
    c, t = _write(tmp_path, _CARGO.replace('0.152.0', '0.152.1'), "rust-v0.152.0\n")
    assert guard.check(c, t) == []


def test_stale_tag_fails(tmp_path):
    # The real 0.149-merge near-miss: workspace bumped, tag left behind.
    c, t = _write(tmp_path, _CARGO.replace('0.152.0', '0.149.0'), "rust-v0.147.0\n")
    failures = guard.check(c, t)
    assert failures and "lags the merged upstream base" in failures[0]


def test_dependency_version_not_mistaken_for_workspace(tmp_path):
    # The gix dependency line (0.83.0) must not be read as the workspace version.
    c, _ = _write(tmp_path, _CARGO, "rust-v0.152.0\n")
    assert guard.workspace_major_minor(c.read_text()) == "0.152"


def test_non_rust_tag_fails(tmp_path):
    c, t = _write(tmp_path, _CARGO, "v0.152.0\n")
    failures = guard.check(c, t)
    assert failures and "not a rust-vX.Y.Z tag" in failures[0]
