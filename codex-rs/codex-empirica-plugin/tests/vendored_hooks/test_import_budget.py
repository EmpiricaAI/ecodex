"""Import-budget gate — the vendored hooks must not eagerly pull heavy deps.

Adapted from empirica's tests/test_import_budget.py (@d1f5dc736, proposal
prop_xhqjdfeafbhutphxagv3pz4h5q).

WHY THIS MATTERS FOR ECODEX
---------------------------
empirica's hot path is `import empirica.cli` (paid once per `empirica <cmd>`).
ecodex's hot path is different: the Rust plugin layer SPAWNS a vendored hook
SCRIPT as a subprocess on every relevant event — sentinel-gate.py on *every*
Bash/Edit/Write (PreToolUse), tool-router.py on every prompt (UserPromptSubmit),
session-init.py / post-compact.py at session start. A heavy import landing at a
hook's module top-level taxes *every* one of those spawns — directly, because
each spawn is a fresh interpreter that re-pays the import.

So the budget here guards the hook spawn latency the same way empirica's guards
its CLI dispatch latency. A new heavy import in a hot hook fails this gate;
widening the budget is then a deliberate, reviewed decision visible in the diff.

THE ADAPTATION — script entry points, not module entry points
-------------------------------------------------------------
empirica budgets importable modules (`empirica.cli`) and measures
`import <module>`. ecodex's hooks are hyphenated SCRIPTS (`sentinel-gate.py`),
not importable as dotted modules. So the measurement loads each script by path
via `importlib.util.spec_from_file_location` + `exec_module` (the same shim
conftest.py / the ai_id test already use), in a fresh subprocess for isolation.
Loading (name != "__main__") runs the module's top-level imports + defs but NOT
its `if __name__ == "__main__"` body — exactly the import cost we want to bound.

PRESENCE-BASED, NOT TIME-BASED
------------------------------
Import *time* is flaky on shared CI; the *cause* we forbid (a heavy module in
`sys.modules`) is deterministic. The budget is "which heavy modules loaded",
measured in a fresh subprocess.

LOAD FAILURES ARE FAILURES
--------------------------
A missing hook or import dependency makes the instrument dead. It must fail
the guard rather than manufacture a pass through ``pytest.skip``.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import pytest

PLUGIN_ROOT = Path(__file__).resolve().parents[2]
HOOKS = PLUGIN_ROOT / "assets" / "hooks_scripts" / "hooks"
LIB = PLUGIN_ROOT / "assets" / "hooks_scripts" / "lib"
EMPIRICA = Path.home() / "empirical-ai" / "empirica"

# Heavy / expensive modules a per-spawn hook should never pull eagerly.
# (LLM/embedding SDKs, network/server stacks, the vector store, ML + data libs,
# the web framework.) None are needed just to gate a tool call or route a prompt.
_HEAVY = frozenset({
    "openai", "anthropic", "voyageai",                 # embedding / LLM SDKs
    "httpx", "uvicorn",                                # network / server
    "git",                                             # GitPython
    "qdrant_client",                                   # vector store
    "torch", "transformers", "sentence_transformers",  # ML
    "pandas", "numpy", "scipy", "sklearn",             # data / science
    "fastapi",                                         # web framework
})

# Per hot-path hook script: the heavy modules it is ALLOWED to pull eagerly
# (its genuine framework). Everything else in _HEAVY is forbidden for that hook.
# This map IS the budget — widening an entry's allow-set is a deliberate decision
# visible in the diff. Empty set = the hook must pull NOTHING heavy at import.
#
# Baseline established by the first audit run (2026-06-21): all critical hooks
# pull nothing heavy at module load (empirica core stays lazy — the CLI's own
# budget upstream is empty too). Kept empty as a strict ratchet.
_BUDGET: dict[str, frozenset[str]] = {
    "sentinel-gate.py": frozenset(),        # PreToolUse — every Bash/Edit/Write
    "tool-router.py": frozenset(),          # UserPromptSubmit — every prompt
    "context-shift-tracker.py": frozenset(),  # UserPromptSubmit
    "session-init.py": frozenset(),         # SessionStart
    "post-compact.py": frozenset(),         # SessionStart / compact
    "pre-compact.py": frozenset(),          # PreCompact
    "session-monitor-arm.py": frozenset(),  # SessionStart
}


def _modules_after_loading_script(script_path: Path) -> tuple[set[str], str | None]:
    """`sys.modules` keys after loading ``script_path`` by path in a fresh interp.

    Returns (loaded_modules, error). On import/load failure error is a string and
    loaded_modules is empty; the caller fails the guard.
    """
    code = (
        "import sys, json, importlib.util\n"
        f"for p in ({str(LIB)!r}, {str(HOOKS)!r}, {str(EMPIRICA)!r}):\n"
        "    if p not in sys.path: sys.path.insert(0, p)\n"
        f"spec = importlib.util.spec_from_file_location('hook_under_test', {str(script_path)!r})\n"
        "mod = importlib.util.module_from_spec(spec)\n"
        "try:\n"
        "    spec.loader.exec_module(mod)\n"
        "except SystemExit:\n"
        "    pass\n"  # hook may sys.exit() at top level in odd payloads — imports already paid
        "print(json.dumps(sorted(sys.modules)))\n"
    )
    proc = subprocess.run(
        [sys.executable, "-c", code],
        capture_output=True,
        text=True,
        stdin=subprocess.DEVNULL,
        timeout=120,
    )
    if proc.returncode != 0:
        return set(), proc.stderr.strip()
    try:
        return set(json.loads(proc.stdout.strip().splitlines()[-1])), None
    except (json.JSONDecodeError, IndexError) as exc:  # pragma: no cover
        return set(), f"could not parse subprocess output: {exc}\n{proc.stdout}"


def _heavy_loaded(forbidden: frozenset[str], loaded: set[str]) -> list[str]:
    """Forbidden top-level modules (or their submodules) present in ``loaded``.

    Matches the top module exactly or any ``mod.`` submodule — so forbidding
    ``git`` (GitPython) never false-matches ``empirica.core.git``.
    """
    return sorted(
        m for m in forbidden
        if m in loaded or any(k == m or k.startswith(m + ".") for k in loaded)
    )


def _load_or_fail(script: Path) -> set[str]:
    loaded, error = _modules_after_loading_script(script)
    if error is not None:
        pytest.fail(f"{script.name} failed to load at module level:\n{error}")
    return loaded


@pytest.mark.parametrize("hook_name", sorted(_BUDGET))
def test_hot_path_hook_stays_within_import_budget(hook_name):
    script = HOOKS / hook_name
    if not script.exists():
        pytest.fail(f"{hook_name} is budgeted but not vendored")

    loaded = _load_or_fail(script)

    allowed = _BUDGET[hook_name]
    forbidden = _HEAVY - allowed
    breached = _heavy_loaded(forbidden, loaded)
    assert not breached, (
        f"{hook_name} eagerly imported heavy module(s) {breached} at module load "
        f"— every hook spawn (this one fires on a hot event) now re-pays that "
        f"cost. Keep them lazy (import inside the function that needs them). If "
        f"the cost is genuinely intentional, add the module to "
        f"_BUDGET['{hook_name}'] with a comment explaining why (widens the budget "
        f"on purpose, in the diff)."
    )


def test_broken_import_is_a_failure_not_a_skip(tmp_path: Path):
    script = tmp_path / "broken-hook.py"
    script.write_text("import verdict_audit_missing_dependency\n", encoding="utf-8")

    with pytest.raises(pytest.fail.Exception, match="failed to load at module level"):
        _load_or_fail(script)


def test_budget_entry_points_are_vendored():
    """Sanity: every budgeted hook is actually vendored — a typo'd key would
    silently never gate anything. Skips (not fails) the ones absent only because
    empirica core can't load, but fails on a key naming a file that doesn't exist."""
    missing = [name for name in _BUDGET if not (HOOKS / name).exists()]
    assert not missing, (
        f"_BUDGET names hook(s) that aren't vendored: {missing}. Fix the key or "
        f"drop it from the budget."
    )


if __name__ == "__main__":
    pytest.main([__file__, "-q", "-s"])
