#!/usr/bin/env python3
"""Drift-guard: assert ecodex's vendored firewall hooks retain critical safety
invariants.

WHY NOT A CONTENT DIFF vs empirica?  setup-codex.py re-vendors empirica's hooks
*genericized* (de-Claude'd: "you (Claude)" → "you", harness-agnostic wording),
so a byte-for-byte diff against empirica is noisy by design and would false-
positive on every run. Instead this guard asserts that the SECURITY-CRITICAL
behaviours are PRESENT in the vendored copy. If a future re-vendor or hand-edit
silently drops one — the exact failure class that nearly shipped a fail-open
firewall this cycle (PR#138 recovery hoist confusion; the v0.2.0 translate
drop) — CI fails loudly instead of silently degrading the differentiator.

Runs in CI with NO empirica dependency (pure file inspection). Extend
INVARIANTS as new firewall-critical fixes land.

Exit 0 = all invariants hold. Exit 1 = at least one missing (prints which).
"""

from __future__ import annotations

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
VENDORED_HOOKS = REPO / "codex-rs" / "codex-empirica-plugin" / "assets" / "hooks_scripts" / "hooks"

# Each invariant: (file, human description, [required substrings — ALL must be present]).
# Keep substrings stable + behaviour-anchored (comment markers + identifiers),
# not line numbers, so genericization / drift doesn't break the guard spuriously.
INVARIANTS: list[tuple[Path, str, list[str]]] = [
    (
        VENDORED_HOOKS / "sentinel-gate.py",
        "recovery escape hatch present (PR#138 rush-guard hoist — recovery/"
        "measurement verbs must never be gated by the firewall they enable)",
        ["RECOVERY ESCAPE HATCH", "is_safe_empirica_command"],
    ),
    (
        VENDORED_HOOKS / "sentinel-gate.py",
        "gate emits codex-native permissionDecision (the field whose translate-"
        "side drop silently broke gating in v0.2.0)",
        ['"permissionDecision"'],
    ),
]


def main() -> int:
    failures: list[str] = []
    checked = 0

    for path, description, required in INVARIANTS:
        checked += 1
        if not path.is_file():
            rel = path.relative_to(REPO)
            failures.append(f"MISSING FILE: {rel} — cannot verify: {description}")
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        missing = [s for s in required if s not in text]
        if missing:
            rel = path.relative_to(REPO)
            failures.append(
                f"INVARIANT VIOLATED in {rel}: {description}\n"
                f"    missing marker(s): {missing}"
            )

    if failures:
        print("✗ vendored firewall drift-guard FAILED:\n", file=sys.stderr)
        for f in failures:
            print(f"  - {f}", file=sys.stderr)
        print(
            "\nA security-critical firewall behaviour is missing from the vendored hooks.\n"
            "This usually means a re-vendor (scripts/setup-codex.py) pulled an empirica\n"
            "revision that lacks the fix, or a hand-edit dropped it. Re-vendor from an\n"
            "empirica ref that carries the fix, or restore the marker, before shipping.",
            file=sys.stderr,
        )
        return 1

    print(f"✓ vendored firewall drift-guard: {checked} invariant(s) hold.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
