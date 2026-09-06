#!/usr/bin/env python3
"""Drift-guard: codex-rs/UPSTREAM_SYNC_TAG must track the merged upstream base.

The release workflow reads UPSTREAM_SYNC_TAG to fetch upstream's prebuilt
codex-code-mode-host for the exact tag we synced from. If the tag file lags the
workspace (as it did after the 0.149.0 merge — the file still said rust-v0.147.0
while main was on 0.149.0, which would have shipped a 0.147 host against the
0.149 runtime protocol), the release ships a mismatched binary. That near-miss
was caught by hand; this guard catches it mechanically.

INVARIANT (purely local, no network, no upstream-tag fetch): the major.minor of
UPSTREAM_SYNC_TAG equals the major.minor of the [workspace.package] version in
codex-rs/Cargo.toml. ecodex's workspace version tracks the upstream base on each
sync (0.149.0, 0.152.0, …); a patch release (0.152.1) stays on the same upstream
base, so minor-equality — not exact-equality — is the correct test: it fails the
stale-tag case (0.147 vs 0.149) while allowing patch releases (0.152.1 vs
rust-v0.152.0).

Pure file inspection; no build, no empirica dependency. Mirrors
check_vendored_firewall.py / check_huggingface_integration.py in shape.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CARGO_TOML = REPO / "codex-rs" / "Cargo.toml"
SYNC_TAG = REPO / "codex-rs" / "UPSTREAM_SYNC_TAG"


def workspace_major_minor(cargo_text: str) -> str:
    """major.minor of the [workspace.package] version — anchored on the section
    header so a dependency's version line can never match by accident."""
    m = re.search(
        r"^\[workspace\.package\][^\[]*?^version\s*=\s*\"(\d+)\.(\d+)\.\d+\"",
        cargo_text,
        re.MULTILINE | re.DOTALL,
    )
    if not m:
        raise ValueError("could not find [workspace.package] version in Cargo.toml")
    return f"{m.group(1)}.{m.group(2)}"


def tag_major_minor(tag_text: str) -> str:
    """major.minor from a `rust-vX.Y.Z` sync tag."""
    tag = tag_text.strip()
    m = re.fullmatch(r"rust-v(\d+)\.(\d+)\.\d+", tag)
    if not m:
        raise ValueError(f"UPSTREAM_SYNC_TAG {tag!r} is not a rust-vX.Y.Z tag")
    return f"{m.group(1)}.{m.group(2)}"


def check(cargo_path: Path = CARGO_TOML, tag_path: Path = SYNC_TAG) -> list[str]:
    failures: list[str] = []
    try:
        ws = workspace_major_minor(cargo_path.read_text(encoding="utf-8"))
        tag = tag_major_minor(tag_path.read_text(encoding="utf-8"))
    except (ValueError, FileNotFoundError) as exc:
        return [str(exc)]
    if ws != tag:
        failures.append(
            f"UPSTREAM_SYNC_TAG minor ({tag}.x) != workspace version minor ({ws}.x) — "
            f"the sync tag lags the merged upstream base. Release CI would fetch a "
            f"codex-code-mode-host for the wrong upstream version. Bump "
            f"codex-rs/UPSTREAM_SYNC_TAG to rust-v{ws}.<z> after the merge."
        )
    return failures


def main() -> int:
    failures = check()
    if failures:
        for f in failures:
            print(f"✗ {f}", file=sys.stderr)
        return 1
    print(
        f"✓ UPSTREAM_SYNC_TAG drift-guard: {SYNC_TAG.read_text().strip()} "
        f"tracks workspace version. OK."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
