"""Self-test for the empirica-core pin drift-guard.

Each failure case has a real failure path — a ref left behind after a
re-vendor (the 2026-09-25 CI break) goes red, a stale version comment goes red,
a missing checkout step goes red — so the guard cannot silently pass.
"""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path

SCRIPT = Path(__file__).with_name("check_empirica_core_pin.py")
_spec = importlib.util.spec_from_file_location("check_empirica_core_pin", SCRIPT)
assert _spec is not None and _spec.loader is not None
guard = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(guard)

_SHA = "13b58943d89e8ea68f0f6c5db778255a5ab869a7"

_CI = f"""jobs:
  vendored-hooks:
    steps:
      - name: Checkout matching Empirica core
        uses: actions/checkout@v6
        with:
          repository: EmpiricaAI/empirica
          ref: {_SHA}  # v1.14.3, matches empiricaVendorCommit
          path: empirica-core
"""


def _write(
    tmp_path: Path, ci: str, commit: str = _SHA[:12], version: str = "1.14.3"
) -> tuple[Path, Path]:
    c = tmp_path / "ci.yml"
    m = tmp_path / "manifest.json"
    c.write_text(ci, encoding="utf-8")
    stamp = {"empiricaVendorVersion": version, "empiricaVendorCommit": commit}
    m.write_text(json.dumps({"name": "empirica", **stamp}), encoding="utf-8")
    return c, m


def test_matching_pin_passes(tmp_path):
    assert guard.check(*_write(tmp_path, _CI)) == []


def test_ref_left_behind_after_revendor_fails(tmp_path):
    failures = guard.check(*_write(tmp_path, _CI, commit="ebd3bb75e000", version="1.14.3"))
    assert len(failures) == 1 and "empiricaVendorCommit" in failures[0]


def test_stale_version_comment_fails(tmp_path):
    failures = guard.check(*_write(tmp_path, _CI.replace("# v1.14.3", "# v1.14.2")))
    assert len(failures) == 1 and "pin comment says v1.14.2" in failures[0]


def test_uncommented_ref_is_judged_by_commit_alone(tmp_path):
    ci = _CI.replace("  # v1.14.3, matches empiricaVendorCommit", "")
    assert guard.check(*_write(tmp_path, ci)) == []


def test_missing_checkout_step_fails(tmp_path):
    ci = _CI.replace("EmpiricaAI/empirica\n", "EmpiricaAI/empirica-cortex\n")
    failures = guard.check(*_write(tmp_path, ci))
    assert len(failures) == 1 and "could not find the empirica-core checkout" in failures[0]


def test_repository_ci_and_manifest_agree():
    assert guard.check() == []
