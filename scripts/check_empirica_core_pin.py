#!/usr/bin/env python3
"""Drift-guard: CI's empirica-core checkout must be the commit ecodex vendored from.

The hermetic vendored-hook job in .github/workflows/ci.yml checks out
EmpiricaAI/empirica at a pinned ref and uses its CLI parser and schema as the
instruments for the vendored hooks. setup-codex.py stamps the commit the hooks
were vendored from into codex-empirica-plugin/manifest.json
(empiricaVendorCommit, the first 12 hex digits, plus empiricaVendorVersion).
Those were two hand-kept copies of one fact, and they drifted: after v0.157.0
the hooks were re-vendored, the pin was not, and CI failed on main
(2026-09-25).

INVARIANT: the empirica-core checkout ref in ci.yml starts with
empiricaVendorCommit, and its trailing `# vX.Y.Z` comment, when present, names
empiricaVendorVersion.

Pure file inspection; no build, no empirica dependency, no YAML parser. Mirrors
check_upstream_sync_tag.py in shape.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CI_YML = REPO / ".github" / "workflows" / "ci.yml"
MANIFEST = REPO / "codex-rs" / "codex-empirica-plugin" / "manifest.json"

# The checkout step's `with:` block: the repository line, then the ref line,
# optionally commented with the release it corresponds to.
_PIN = re.compile(
    r"^\s*repository:\s*EmpiricaAI/empirica\s*\n"
    r"\s*ref:\s*(?P<ref>[0-9a-f]{40})\b[^\n#]*(?:#\s*v(?P<version>\d+\.\d+\.\d+))?",
    re.MULTILINE,
)


def ci_pin(ci_text: str) -> tuple[str, str | None]:
    """The pinned empirica-core ref in ci.yml and the version its comment names."""
    m = _PIN.search(ci_text)
    if not m:
        raise ValueError(
            "could not find the empirica-core checkout in ci.yml "
            "(`repository: EmpiricaAI/empirica` followed by `ref: <40-hex sha>`)"
        )
    return m.group("ref"), m.group("version")


def vendor_stamp(manifest_text: str) -> tuple[str, str]:
    """empiricaVendorCommit and empiricaVendorVersion from the plugin manifest."""
    manifest = json.loads(manifest_text)
    try:
        return manifest["empiricaVendorCommit"], manifest["empiricaVendorVersion"]
    except KeyError as exc:
        raise ValueError(f"manifest.json has no {exc.args[0]}") from None


def check(ci_path: Path = CI_YML, manifest_path: Path = MANIFEST) -> list[str]:
    try:
        ref, comment_version = ci_pin(ci_path.read_text(encoding="utf-8"))
        commit, version = vendor_stamp(manifest_path.read_text(encoding="utf-8"))
    except (ValueError, FileNotFoundError) as exc:
        return [str(exc)]
    failures: list[str] = []
    if not ref.startswith(commit):
        failures.append(
            f"ci.yml checks out empirica-core at {ref[:12]}, but the vendored hooks "
            f"come from {commit} (manifest.json empiricaVendorCommit). The hook "
            f"tests would measure the vendored hooks against the wrong empirica. "
            f"Set the ref in .github/workflows/ci.yml to the full sha of {commit}."
        )
    if comment_version is not None and comment_version != version:
        failures.append(
            f"ci.yml's pin comment says v{comment_version}, but manifest.json "
            f"empiricaVendorVersion is {version}. Update the comment with the ref."
        )
    return failures


def main() -> int:
    failures = check()
    if failures:
        for f in failures:
            print(f"✗ {f}", file=sys.stderr)
        return 1
    ref, _ = ci_pin(CI_YML.read_text(encoding="utf-8"))
    print(f"✓ empirica-core pin guard: ci.yml checks out {ref[:12]}, the vendored commit. OK.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
