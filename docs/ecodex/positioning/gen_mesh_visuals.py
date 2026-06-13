#!/usr/bin/env python3
"""Generate the 'worktree-style MAS vs federated calibrated practices' visuals.

Renders one SVG per slide via asciisvg (mdview's DiagramSpec -> render_spec_svg),
plus a self-contained index.html for review. The SVGs double as slide figures;
pair them with notebooklm-slidedeck-prompts.md to produce the deck.

Run:  python3 gen_mesh_visuals.py
Dep:  pip install asciisvg   (falls back to ~/empirical-ai/mdview/src if absent)
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

try:
    from mdview.spec import DiagramSpec, Element, Connection
    from mdview.specrender import render_spec_svg
except ModuleNotFoundError:  # not pip-installed — use local checkout
    src = os.environ.get("MDVIEW_SRC", str(Path.home() / "empirical-ai" / "mdview" / "src"))
    sys.path.insert(0, src)
    from mdview.spec import DiagramSpec, Element, Connection
    from mdview.specrender import render_spec_svg

OUT = Path(__file__).resolve().parent


# ── Slide 1 — worktree-style MAS (today) ────────────────────────────
def worktree_mas() -> DiagramSpec:
    return DiagramSpec(
        type="box",
        layout="horizontal",
        title="Today: worktree-style multi-agent",
        elements=[
            Element(id="orch", label="orchestrator", type="box",
                    properties={"sections": [["one repo", "one machine", "one authority"]]}),
            Element(id="w1", label="worker 1", type="box",
                    properties={"sections": [["ephemeral", "clone", "no memory"]]}),
            Element(id="w2", label="worker 2", type="box",
                    properties={"sections": [["ephemeral", "clone", "no memory"]]}),
            Element(id="w3", label="worker 3", type="box",
                    properties={"sections": [["ephemeral", "clone", "no memory"]]}),
        ],
        connections=[
            Connection(from_id="orch", to_id="w1", label="fan-out (worktree)"),
            Connection(from_id="orch", to_id="w2", label="fan-out"),
            Connection(from_id="orch", to_id="w3", label="fan-out"),
        ],
    )


# ── Slide 2 — federated calibrated practices (ours) ─────────────────
def federated_practices() -> DiagramSpec:
    seat_sections = {"sections": [[
        "own repo + history",
        "calibration trajectory",
        "practitioner [model] swappable",
    ]]}
    return DiagramSpec(
        type="box",
        layout="horizontal",
        title="Ours: federation of calibrated practices",
        elements=[
            Element(id="A", label="practice A (seat)", type="box", properties=seat_sections),
            Element(id="B", label="practice B (seat)", type="box", properties=seat_sections),
            Element(id="C", label="practice C (seat)", type="box", properties=seat_sections),
        ],
        connections=[
            Connection(from_id="A", to_id="B", label="collab (open / ungated)"),
            Connection(from_id="B", to_id="C", label="propose (+ECO gate)"),
        ],
    )


# ── Slide 3 — trust rings (governance scales with distance) ─────────
def trust_rings() -> DiagramSpec:
    return DiagramSpec(
        type="wireframe",
        layout="nested",
        title="Governance tightens with trust distance",
        elements=[
            Element(id="crossorg", label="cross-org   - gate: L3 ECO + System tab",
                    type="panel", children=["org"]),
            Element(id="org", label="org   - gate: cross-org escalation",
                    type="panel", children=["tenant"]),
            Element(id="tenant", label="tenant   - gate: cross-tenant review",
                    type="panel", children=["practice"]),
            Element(id="practice", label="practice (seat)   - collab: open  |  propose: ECO",
                    type="panel", children=["practitioner"]),
            Element(id="practitioner", label="practitioner [model] - swappable, fungible",
                    type="panel"),
        ],
    )


# ── Slide 4 — practice vs worker (the lifetime axis) ────────────────
def practice_vs_worker() -> DiagramSpec:
    return DiagramSpec(
        type="box",
        layout="horizontal",
        title="The unit: ephemeral worker vs persistent seat",
        elements=[
            Element(id="worker", label="WORKER (today)", type="box",
                    properties={"sections": [[
                        "spawn -> work -> die",
                        "respawns fresh each task",
                        "no identity, no memory",
                        "leaves no trace",
                    ]]}),
            Element(id="practice", label="PRACTICE (ours)", type="box",
                    properties={"sections": [[
                        "birth ---> now (continuous)",
                        "calibration trajectory persists",
                        "identity lives in the SEAT",
                        "model [a]->[b]->[c] swaps in",
                        "every belief logged + gated",
                    ]]}),
        ],
    )


# ── Slide 5 — comparison table ──────────────────────────────────────
def comparison() -> DiagramSpec:
    return DiagramSpec(
        type="table",
        title="worktree-style MAS  vs  federated practices",
        properties={"columns": 3},
        elements=[
            Element(id="h", label="", type="header",
                    properties={"cells": ["", "worktree-style MAS", "federated practices"]}),
            Element(id="r1", label="", type="row",
                    properties={"cells": ["unit", "N checkouts of ONE repo", "N independent practices"]}),
            Element(id="r2", label="", type="row",
                    properties={"cells": ["authority", "single object store", "federated, each sovereign"]}),
            Element(id="r3", label="", type="row",
                    properties={"cells": ["spans machines/orgs", "no (local FS)", "yes, governed"]}),
            Element(id="r4", label="", type="row",
                    properties={"cells": ["lifetime", "ephemeral, dies at task", "persistent seat + trajectory"]}),
            Element(id="r5", label="", type="row",
                    properties={"cells": ["identity", "none (clones)", "in the seat; model swaps in"]}),
            Element(id="r6", label="", type="row",
                    properties={"cells": ["carries", "code diffs", "calibrated belief + gated action"]}),
            Element(id="r7", label="", type="row",
                    properties={"cells": ["solves", "execution isolation", "coordination + governance"]}),
        ],
    )


SLIDES = [
    ("01-worktree-mas", "Today: worktree-style multi-agent", worktree_mas),
    ("02-federated-practices", "Ours: federation of calibrated practices", federated_practices),
    ("03-trust-rings", "Governance tightens with trust distance", trust_rings),
    ("04-practice-vs-worker", "Ephemeral worker vs persistent seat", practice_vs_worker),
    ("05-comparison", "Side-by-side comparison", comparison),
]


def main() -> None:
    cards = []
    for slug, title, fn in SLIDES:
        svg = render_spec_svg(fn())
        (OUT / f"{slug}.svg").write_text(svg, encoding="utf-8")
        cards.append(f'<section><h2>{title}</h2>\n{svg}\n</section>')
        print(f"wrote {slug}.svg ({len(svg)} bytes)")

    html = (
        "<!doctype html><meta charset=utf-8>"
        "<title>Mesh vs worktree-MAS</title>"
        "<style>body{font-family:system-ui;max-width:980px;margin:2rem auto;"
        "padding:0 1rem}section{margin:2.5rem 0;border-bottom:1px solid #ddd;"
        "padding-bottom:1.5rem}h2{font-size:1.1rem}</style>"
        "<h1>Worktree-style MAS vs federated calibrated practices</h1>"
        + "\n".join(cards)
    )
    (OUT / "index.html").write_text(html, encoding="utf-8")
    print(f"wrote index.html ({len(html)} bytes) -> open in a browser to review")


if __name__ == "__main__":
    main()
