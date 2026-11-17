"""The bootstrap renderer (lib/context_render.py) preserves the whole contract.

The budget test beside this one (test_session_context_budget.py) proves the
rendered block stays under codex's hook cap and does not grow with the history.
This file is the other half of the prop_b4454 integration: it feeds
``context_render.render`` fixtures for every artifact category and every coverage
and blindspot *state*, and asserts the contract the extension asked ecodex to
preserve through startup and compaction recovery —

  - all six categories (findings, unknowns, decisions, assumptions, mistakes
    with prevention, dead ends) plus bounded artifact relationships, each with a
    stable 8-char id;
  - a coverage line that distinguishes, per category: nothing logged, a capped
    source, a source that did not answer (unavailable), and a category this core
    did not return — never collapsing any of these to a bare "none";
  - blindspot status that distinguishes not-read (error), not-read (store did
    not answer), scanned counts, and none recorded;
  - "project loaded, nothing retrieved" kept distinct from "no context"
    (bootstrap did not answer);
  - the budget enforced, with coverage still naming every category;
  - the post-compact ``only=`` path: only the complementary categories render
    item lines, while coverage still names the ranked ones.

Pure renderer in, text out — no hook, no I/O — so the fixtures are the contract.
Running this file green is the end-to-end evidence for prop_b4454.
"""

from __future__ import annotations

import importlib.util
import math
import sys
from pathlib import Path

_LIB = Path(__file__).resolve().parents[2] / "assets" / "hooks_scripts" / "lib"

# The eight bootstrap keys, and the names they read as in the coverage line.
_CATS = ("goals", "findings", "unknowns", "decisions", "assumptions",
         "mistakes_to_avoid", "dead_ends", "artifact_edges")
_COV_NAMES = ("goals", "findings", "unknowns", "decisions", "assumptions",
              "mistakes to avoid", "dead ends", "links")
_HEADINGS = ("Goals", "Findings", "Unknowns", "Decisions", "Assumptions",
             "Mistakes to avoid", "Dead ends", "Links")
_ITEM_CATS = ("goals", "findings", "unknowns", "decisions", "assumptions",
              "mistakes_to_avoid", "dead_ends")


def _load_context_render():
    sys.path.insert(0, str(_LIB))
    try:
        sys.modules.pop("context_render_contract_test", None)
        spec = importlib.util.spec_from_file_location(
            "context_render_contract_test", _LIB / "context_render.py"
        )
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module
    finally:
        sys.path.pop(0)


CR = _load_context_render()


def _tokens(text: str) -> int:
    return math.ceil(len(text.encode("utf-8")) / 4)


def _field(key: str) -> str:
    return {"goals": "objective", "findings": "finding", "unknowns": "unknown",
            "decisions": "choice", "assumptions": "assumption",
            "mistakes_to_avoid": "mistake", "dead_ends": "approach"}[key]


def _coverage_line(out: str) -> str:
    return next(line for line in out.splitlines() if line.startswith("Coverage:"))


def _full_breadcrumbs(n: int = 3) -> dict:
    """Every category populated, each with a sqlite coverage entry of total 3n."""

    def rows(field: str) -> list[dict]:
        return [{field: f"{i}: a real artifact line", "id": f"{field[:3]}{i:05d}dead"}
                for i in range(n)]

    bc: dict = {
        "goals": [{"objective": f"goal {i}", "id": f"g{i:05d}", "status": "in_progress"}
                  for i in range(n)],
        "findings": [{"finding": f"finding {i}", "id": f"f{i:05d}", "impact": 0.7}
                     for i in range(n)],
        "unknowns": rows("unknown"),
        "decisions": rows("choice"),
        "assumptions": [{"assumption": f"assume {i}", "id": f"a{i:05d}", "confidence": 0.6}
                        for i in range(n)],
        "mistakes_to_avoid": [{"mistake": f"mistake {i}", "prevention": f"prevent {i}"}
                              for i in range(n)],
        "dead_ends": [{"approach": f"approach {i}", "why_failed": f"because {i}", "id": f"d{i:05d}"}
                      for i in range(n)],
        "artifact_edges": [{"from_type": "finding", "from": f"f{i:05d}", "relation": "evidence",
                            "to_type": "decision", "to": f"c{i:05d}"} for i in range(n)],
        "coverage": {k: {"source": "sqlite", "total": 3 * n} for k in _CATS},
        "blindspot_status": {"surfaced": 2, "acknowledged": 5, "dismissed": 1},
    }
    return bc


def test_every_category_renders_with_its_heading_and_a_stable_id():
    out = CR.render(_full_breadcrumbs(), budget=100_000)
    for heading in _HEADINGS:
        assert f"**{heading}:**" in out, f"{heading} section missing: no category may be dropped"
    # Stable 8-char id prefix survives (findings carry f00000..); verbs take it.
    assert "[f00000" in out, "finding id prefix must render"
    assert "Ids are 8-character prefixes" in out
    # Mistake carries its prevention (the load-bearing half).
    assert "→ prevent 0" in out


def test_coverage_counts_every_category_when_populated():
    cov = _coverage_line(CR.render(_full_breadcrumbs(n=3), budget=100_000))
    for name in _COV_NAMES:
        assert name in cov, f"coverage omits {name}"
    assert "of 9" in cov  # total 3*n = 9


def test_empty_project_is_distinct_from_no_context():
    # No context: bootstrap did not answer at all.
    assert "No context available" in CR.render(None)
    # Project loaded but every category empty: a different, explicit statement.
    empty: dict = {k: [] for k in _CATS}
    empty["coverage"] = {k: {"source": "sqlite", "total": 0} for k in _CATS}
    out = CR.render(empty, budget=100_000)
    assert "nothing retrieved" in out
    assert "No context available" not in out
    assert "findings: none" in _coverage_line(out)


def test_unavailable_source_is_not_reported_as_none():
    bc = _full_breadcrumbs(n=0)
    # findings store did not answer; unknowns is a true empty.
    bc["coverage"]["findings"] = {"source": "unavailable"}
    bc["coverage"]["unknowns"] = {"source": "sqlite", "total": 0}
    cov = _coverage_line(CR.render(bc, budget=100_000))
    assert "findings: unavailable" in cov, "an unanswered source must not read as 'none'"
    assert "unknowns: none" in cov
    assert "findings: none" not in cov


def test_capped_source_says_capped():
    bc = _full_breadcrumbs(n=0)
    bc["coverage"]["findings"] = {"source": "sqlite", "total": 50, "truncated": True}
    assert "findings: none returned (capped)" in _coverage_line(CR.render(bc, budget=100_000))


def test_category_not_returned_by_core():
    bc = _full_breadcrumbs(n=2)
    del bc["assumptions"]  # an older core that never returned this category
    assert "assumptions: not returned by this core" in _coverage_line(CR.render(bc, budget=100_000))


def test_blindspot_states_are_distinct():
    base = _full_breadcrumbs(n=0)

    def blind(status) -> str:
        bc = dict(base)
        bc["blindspot_status"] = status
        return next((line for line in CR.render(bc, budget=100_000).splitlines()
                     if line.startswith("Blindspots:")), "")

    assert "not read (qdrant down)" in blind({"error": "qdrant down"})
    assert "did not answer" in blind({"source": "unavailable"})
    assert "3 surfaced" in blind({"surfaced": 3, "acknowledged": 1})
    assert "none recorded" in blind({})
    # Not a dict → no blindspot line at all.
    bc = dict(base)
    bc["blindspot_status"] = None
    lines = CR.render(bc, budget=100_000).splitlines()
    assert not any(ln.startswith("Blindspots:") for ln in lines)


def test_budget_is_enforced_but_coverage_names_every_category():
    big: dict = {k: [{_field(k): "x" * 400, "id": f"{k[:3]}{i:05d}"} for i in range(5_000)]
                 for k in _ITEM_CATS}
    big["artifact_edges"] = [{"from": "a", "to": "b", "relation": "evidence"}] * 5_000
    big["coverage"] = {k: {"source": "sqlite", "total": 5_000} for k in _CATS}
    big["blindspot_status"] = {"surfaced": 1}
    out = CR.render(big, budget=1_800)
    assert _tokens(out) <= 1_800 + 120, "render must choose under budget, not emit all history"
    cov = _coverage_line(out)
    for name in _COV_NAMES:
        assert name in cov, f"budget truncation dropped {name} from coverage"


def test_post_compact_only_path_renders_complement_but_coverage_names_all():
    only = {"decisions", "assumptions", "mistakes_to_avoid", "artifact_edges"}
    out = CR.render(_full_breadcrumbs(n=3), budget=100_000, only=only)
    assert "**Decisions:**" in out and "**Assumptions:**" in out
    # The ranked-elsewhere categories render no item sections here...
    assert "**Findings:**" not in out
    assert "**Unknowns:**" not in out
    assert "**Goals:**" not in out
    # ...but coverage still names them, pointing at the focus above (not "0 of N").
    cov = _coverage_line(out)
    assert "findings 9, ranked with every type in the focus above" in cov
    assert "0 of" not in cov  # the 1.16.2 sweep-A3 regression must not return
