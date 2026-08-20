#!/usr/bin/env python3
"""Verify executable firewall invariants in the vendored Sentinel hook.

This guard deliberately inspects Python structure rather than searching source
text. Comments and dead marker strings are not evidence that a safety path is
live.
"""

from __future__ import annotations

import ast
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SENTINEL = (
    REPO
    / "codex-rs"
    / "codex-empirica-plugin"
    / "assets"
    / "hooks_scripts"
    / "hooks"
    / "sentinel-gate.py"
)


def _function(tree: ast.Module, name: str) -> ast.FunctionDef | None:
    return next(
        (node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == name),
        None,
    )


def _recovery_escape_is_hoisted(tree: ast.Module) -> bool:
    function = _function(tree, "_validate_check_record")
    if function is None:
        return False
    body = function.body
    if ast.get_docstring(function, clean=False) is not None:
        body = body[1:]
    first = body[0] if body else None
    if not isinstance(first, ast.If):
        return False
    calls = {
        node.func.id
        for node in ast.walk(first.test)
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Name)
    }
    returns_none = any(
        isinstance(node, ast.Return)
        and (
            node.value is None
            or (isinstance(node.value, ast.Constant) and node.value.value is None)
        )
        for statement in first.body
        for node in ast.walk(statement)
    )
    return "is_safe_empirica_command" in calls and returns_none


def _permission_decision_uses_argument(tree: ast.Module) -> bool:
    function = _function(tree, "respond")
    if function is None:
        return False
    return any(
        isinstance(key, ast.Constant)
        and key.value == "permissionDecision"
        and isinstance(value, ast.Name)
        and value.id == "decision"
        for node in ast.walk(function)
        if isinstance(node, ast.Dict)
        for key, value in zip(node.keys, node.values, strict=True)
    )


def _any_if_test_contains_literal_false(tree: ast.Module) -> bool:
    """A disabled-in-place path (`if False and (...)`) keeps every marker and
    every structural node while removing the behaviour — demonstrated live
    against this guard's structural checks (2026-08-21, exit 0 with the
    escape hatch dead). Any literal False inside an `if` test is dead code at
    best and a disabled security path at worst; either way it must not pass."""
    for node in ast.walk(tree):
        if isinstance(node, ast.If):
            for sub in ast.walk(node.test):
                if isinstance(sub, ast.Constant) and sub.value is False:
                    return True
    return False


def check(path: Path = SENTINEL) -> list[str]:
    if not path.is_file():
        return [f"missing vendored Sentinel hook: {path}"]
    try:
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    except (OSError, UnicodeError, SyntaxError) as exc:
        return [f"cannot inspect vendored Sentinel hook: {exc}"]

    failures: list[str] = []
    if not _recovery_escape_is_hoisted(tree):
        failures.append(
            "_validate_check_record must begin with an executable recovery escape "
            "that calls is_safe_empirica_command and returns None"
        )
    if not _permission_decision_uses_argument(tree):
        failures.append("respond() must emit permissionDecision from its decision argument")
    if _any_if_test_contains_literal_false(tree):
        failures.append(
            "an `if` test contains a literal False — a disabled-in-place code "
            "path defeats structural checks while removing behaviour"
        )
    return failures


def main() -> int:
    failures = check()
    if failures:
        print("✗ vendored firewall drift-guard FAILED:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1
    print("✓ vendored firewall drift-guard: 2 executable invariants hold.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
