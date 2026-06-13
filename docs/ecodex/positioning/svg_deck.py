#!/usr/bin/env python3
"""Presentation-grade SVG deck: worktree-style MAS vs federated calibrated practices.

Hand-built (not asciisvg) for slide quality: fixed 16:9 canvas, design system,
real concentric trust-rings, split-column lifetime view, model chips, styled
table. stdlib only. Run:  python3 svg_deck.py   ->  deck/*.svg + deck/index.html
"""
from __future__ import annotations

import html as _html
from pathlib import Path

W, H = 1280, 720
OUT = Path(__file__).resolve().parent / "deck"

# ── Design system ───────────────────────────────────────────────────
FONT = "-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif"
BG, INK, MUTED, LINE, PANEL = "#ffffff", "#0f172a", "#64748b", "#cbd5e1", "#f8fafc"
OLD, OLD_FILL, OLD_INK = "#94a3b8", "#f1f5f9", "#475569"          # worktree / "the old way"
TEAL, TEAL_D, TEAL_FILL, TEAL_TINT = "#0d9488", "#0f766e", "#ccfbf1", "#f0fdfa"  # ours
VIOLET, VIOLET_FILL = "#7c3aed", "#ede9fe"                         # governance / gate
AMBER, AMBER_FILL, AMBER_INK = "#f59e0b", "#fef3c7", "#92400e"     # swappable model
RED = "#ef4444"


def esc(s: str) -> str:
    return _html.escape(str(s))


def T(x, y, s, size: float = 16, weight=400, fill=INK, anchor="start", italic=False):
    st = f"font-family:{FONT};font-size:{size}px;font-weight:{weight};fill:{fill}"
    if italic:
        st += ";font-style:italic"
    return f'<text x="{x:.1f}" y="{y:.1f}" text-anchor="{anchor}" style="{st}">{esc(s)}</text>'


def card(x, y, w, h, fill=PANEL, stroke=LINE, sw=1.5, rx=14, shadow=True, dash=None):
    f = ' filter="url(#sh)"' if shadow else ""
    d = f' stroke-dasharray="{dash}"' if dash else ""
    return (f'<rect x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{h:.1f}" rx="{rx}" '
            f'fill="{fill}" stroke="{stroke}" stroke-width="{sw}"{d}{f}/>')


def circ(cx, cy, r, fill="none", stroke=LINE, sw: float = 2):
    return f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{r:.1f}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>'


def dot(cx, cy, r=5, fill=TEAL, stroke="none"):
    return f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{r}" fill="{fill}" stroke="{stroke}" stroke-width="2"/>'


def arrow(x1, y1, x2, y2, color=TEAL, sw=2.2, dash=None, head="t"):
    d = f' stroke-dasharray="{dash}"' if dash else ""
    return (f'<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" '
            f'stroke="{color}" stroke-width="{sw}"{d} marker-end="url(#ah-{head})"/>')


def cross(cx, cy, s=7, color=RED, sw=3):
    return (f'<line x1="{cx-s}" y1="{cy-s}" x2="{cx+s}" y2="{cy+s}" stroke="{color}" stroke-width="{sw}" stroke-linecap="round"/>'
            f'<line x1="{cx-s}" y1="{cy+s}" x2="{cx+s}" y2="{cy-s}" stroke="{color}" stroke-width="{sw}" stroke-linecap="round"/>')


def chip(cx, cy, label="model", fill=AMBER_FILL, stroke=AMBER, ink=AMBER_INK):
    w = len(label) * 7.0 + 26
    return (card(cx - w / 2, cy - 14, w, 28, fill, stroke, 1.3, 14, shadow=False)
            + T(cx, cy + 4, label, 12.5, 600, ink, "middle"))


def _defs():
    heads = "".join(
        f'<marker id="ah-{k}" viewBox="0 0 10 10" refX="8.5" refY="5" markerWidth="7" '
        f'markerHeight="7" orient="auto-start-reverse"><path d="M0,1 L9,5 L0,9 Z" fill="{c}"/></marker>'
        for k, c in (("t", TEAL), ("v", VIOLET), ("s", OLD), ("d", MUTED))
    )
    return ("<defs>"
            '<filter id="sh" x="-20%" y="-20%" width="140%" height="140%">'
            '<feDropShadow dx="0" dy="2" stdDeviation="5" flood-color="#0f172a" flood-opacity="0.10"/></filter>'
            + heads + "</defs>")


def frame(title, subtitle, footer, body, accent=TEAL):
    return "\n".join([
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">',
        _defs(),
        f'<rect width="{W}" height="{H}" fill="{BG}"/>',
        f'<rect x="0" y="0" width="{W}" height="6" fill="{accent}"/>',
        T(64, 74, title, 31, 700, INK),
        T(64, 106, subtitle, 17, 400, MUTED),
        body,
        f'<line x1="64" y1="{H-58}" x2="{W-64}" y2="{H-58}" stroke="{LINE}" stroke-width="1"/>',
        T(64, H - 32, footer, 14.5, 400, MUTED, italic=True),
        "</svg>",
    ])


# ── Slide 1 — worktree-style MAS ────────────────────────────────────
def s1_worktree():
    b = [card(80, 150, 1120, 410, OLD_FILL, OLD, 1.5, 18)]
    b.append(T(104, 186, "ONE repo  ·  ONE machine  ·  ONE authority (shared object store)", 15, 700, OLD_INK))
    # orchestrator
    ox, oy, ow, oh = 510, 210, 260, 64
    b.append(card(ox, oy, ow, oh, "#ffffff", OLD, 2, 12))
    b.append(T(ox + ow / 2, oy + oh / 2 + 5, "orchestrator", 18, 700, OLD_INK, "middle"))
    # workers
    centers = [310, 640, 970]
    wy, wh, ww = 400, 130, 240
    for cx in centers:
        b.append(arrow(640, oy + oh, cx, wy, OLD, 2, head="s"))
    for cx in centers:
        x = cx - ww / 2
        b.append(card(x, wy, ww, wh, "#ffffff", OLD, 1.6, 12, dash="7 6"))
        b.append(T(cx, wy + 34, "worker", 17, 700, OLD_INK, "middle"))
        b.append(T(cx, wy + 62, "ephemeral · clone", 13.5, 400, MUTED, "middle"))
        b.append(T(cx, wy + 84, "no memory", 13.5, 400, MUTED, "middle"))
        b.append(cross(cx, wy + 108, 6))
        b.append(T(cx + 14, wy + 112, "dies at task end", 11.5, 400, MUTED, "start"))
    foot = "Solves EXECUTION ISOLATION — each worker gets a clean checkout so it can't stomp the others. It carries code; it knows nothing about what the others decided."
    return frame("Today — worktree-style multi-agent",
                 "The common pattern: an orchestrator fans out N isolated git worktrees.",
                 foot, "\n".join(b), accent=OLD)


# ── Slide 2 — federated practices ───────────────────────────────────
def s2_federated():
    b = []
    xs = [80, 490, 900]
    names = ["practice A", "practice B", "practice C"]
    cw, ch, cy = 300, 380, 160
    for x, nm in zip(xs, names):
        b.append(card(x, cy, cw, ch, TEAL_TINT, TEAL, 2, 16))
        b.append(T(x + 20, cy + 36, nm, 19, 700, TEAL_D))
        b.append(T(x + 20, cy + 58, "(a seat)", 13, 400, MUTED))
        b.append(chip(x + cw - 64, cy + 30, "model"))
        b.append(T(x + cw - 64, cy + 60, "swappable", 11, 500, MUTED, "middle"))
        # trajectory tail
        ty = cy + 110
        b.append(T(x + 20, ty - 14, "calibration trajectory", 11.5, 600, MUTED))
        b.append(f'<line x1="{x+24}" y1="{ty}" x2="{x+cw-24}" y2="{ty}" stroke="{TEAL}" stroke-width="2"/>')
        for i in range(5):
            px = x + 28 + i * (cw - 56) / 4
            b.append(dot(px, ty, 4 if i < 4 else 6, "#ffffff" if i == 0 else TEAL, TEAL))
        b.append(T(x + 24, ty + 22, "birth", 10.5, 400, MUTED))
        b.append(T(x + cw - 24, ty + 22, "now", 10.5, 600, TEAL_D, "end"))
        # bullets
        by = cy + 180
        for line in ["own repo + history", "own calibration", "identity in the seat"]:
            b.append(dot(x + 26, by - 4, 3, TEAL))
            b.append(T(x + 38, by, line, 14, 400, INK))
            by += 30
    # edges
    midy = cy + ch / 2 + 40
    b.append(arrow(380, midy, 490, midy, TEAL, 2.4, head="t"))
    b.append(T(435, midy - 12, "collab", 13.5, 700, TEAL_D, "middle"))
    b.append(T(435, midy + 22, "open · ungated", 11.5, 400, MUTED, "middle"))
    b.append(arrow(790, midy, 900, midy, VIOLET, 2.4, head="v"))
    b.append(T(845, midy - 12, "propose", 13.5, 700, VIOLET, "middle"))
    b.append(T(845, midy + 22, "ECO-gated", 11.5, 400, MUTED, "middle"))
    foot = "Sovereign: each practice has its own repo, history, calibration — possibly a different machine, user, substrate, or org. They coordinate by message, not by a shared filesystem."
    return frame("Ours — a federation of calibrated practices",
                 "collab carries knowledge (can't act on you, so ungated). propose carries action (gated).",
                 foot, "\n".join(b))


# ── Slide 3 — trust rings (concentric) ──────────────────────────────
def s3_rings():
    cx, cy = 470, 392
    rings = [
        (300, VIOLET_FILL, VIOLET, 5.0, "cross-org  —  L3 ECO + System tab"),
        (245, "#f5f3ff", VIOLET, 4.0, "org"),
        (185, TEAL_TINT, TEAL, 3.0, "tenant  —  cross-tenant: reviewed"),
        (120, TEAL_FILL, TEAL, 2.4, "practice  —  collab: open · propose: ECO"),
    ]
    b = []
    for r, fill, stroke, sw, _ in rings:           # outer → inner so inner overlays
        b.append(circ(cx, cy, r, fill, stroke, sw))
    b.append(circ(cx, cy, 58, AMBER_FILL, AMBER, 2.4))
    b.append(T(cx, cy - 4, "practitioner", 14, 700, AMBER_INK, "middle"))
    b.append(T(cx, cy + 16, "[model] swappable", 11.5, 500, AMBER_INK, "middle"))
    for r, _f, stroke, _sw, label in rings:        # labels along each ring's top
        b.append(card(cx - len(label) * 3.4 - 8, cy - r - 1, len(label) * 6.8 + 16, 22, BG, "none", 0, 6, shadow=False))
        b.append(T(cx, cy - r + 14, label, 12.5, 700, stroke, "middle"))
    # legend
    lx, ly = 880, 250
    b.append(card(lx, ly, 320, 250, PANEL, LINE, 1.4, 14))
    b.append(T(lx + 20, ly + 34, "Reading the rings", 16, 700, INK))
    items = [
        ("stroke weight = gate strength", MUTED),
        ("inner = open · outer = hard-gated", MUTED),
        ("identity is canonical, server-resolved", MUTED),
        ("(senders can't be spoofed)", MUTED),
    ]
    yy = ly + 66
    for s, c in items:
        b.append(dot(lx + 24, yy - 4, 3, TEAL))
        b.append(T(lx + 36, yy, s, 13.5, 400, c))
        yy += 30
    for i, sw in enumerate((1.5, 3, 5)):
        b.append(f'<line x1="{lx+28}" y1="{yy+i*22}" x2="{lx+92}" y2="{yy+i*22}" stroke="{VIOLET}" stroke-width="{sw}"/>')
        lbl = ["practice", "tenant", "cross-org"][i]
        b.append(T(lx + 104, yy + i * 22 + 4, lbl, 12.5, 500, INK))
    foot = "Every actionable message traces to an ECO decision (the auth boundary). collab flows free; propose is gated. The agentic-web failure modes — injection cascades, spoofing, runaway action — don't land."
    return frame("Governance tightens with trust distance",
                 "Not an open agent-internet — a private federation where the gate hardens with distance.",
                 foot, "\n".join(b), accent=VIOLET)


# ── Slide 4 — seat vs worker ────────────────────────────────────────
def s4_lifetime():
    b = []
    # left: worker
    lx, ly, lw, lh = 80, 150, 520, 410
    b.append(card(lx, ly, lw, lh, OLD_FILL, OLD, 1.6, 16))
    b.append(T(lx + 24, ly + 38, "WORKER (today)", 18, 700, OLD_INK))
    cyc_x = lx + 90
    for i in range(3):
        top = ly + 90 + i * 100
        b.append(dot(cyc_x, top, 6, "#ffffff", OLD))
        b.append(T(cyc_x + 18, top + 5, "spawn", 13.5, 600, OLD_INK))
        b.append(f'<line x1="{cyc_x}" y1="{top+10}" x2="{cyc_x}" y2="{top+46}" stroke="{OLD}" stroke-width="2" stroke-dasharray="4 4"/>')
        b.append(T(cyc_x + 18, top + 32, "work", 13, 400, MUTED))
        b.append(cross(cyc_x, top + 58, 6))
        b.append(T(cyc_x + 18, top + 62, "die  (no trace)", 13, 400, MUTED))
    b.append(T(lx + 24, ly + lh - 24, "no identity · no memory · respawns fresh", 13.5, 600, OLD_INK))
    # right: practice
    rx, ry, rw, rh = 680, 150, 520, 410
    b.append(card(rx, ry, rw, rh, TEAL_TINT, TEAL, 2, 16))
    b.append(T(rx + 24, ry + 38, "PRACTICE (ours)", 18, 700, TEAL_D))
    tl_x = rx + 70
    top, bot = ry + 86, ry + 330
    b.append(f'<line x1="{tl_x}" y1="{top}" x2="{tl_x}" y2="{bot}" stroke="{TEAL}" stroke-width="2.5"/>')
    b.append(dot(tl_x, top, 7, "#ffffff", TEAL))
    b.append(T(tl_x + 16, top + 5, "birth", 13.5, 600, TEAL_D))
    for i in range(1, 5):
        b.append(dot(tl_x, top + i * (bot - top) / 5, 4, TEAL))
    b.append(dot(tl_x, bot, 8, TEAL, TEAL))
    b.append(T(tl_x + 16, bot + 5, "now  (continuous)", 13.5, 600, TEAL_D))
    # seat + model swaps
    sx = rx + 250
    b.append(card(sx, ry + 110, 210, 70, "#ffffff", TEAL, 1.8, 12))
    b.append(T(sx + 105, ry + 150, "the SEAT", 16, 700, TEAL_D, "middle"))
    for i, m in enumerate(("model A", "model B", "model C")):
        b.append(chip(sx + 50 + i * 80, ry + 230, m))
        if i < 2:
            b.append(arrow(sx + 50 + i * 80 + 30, ry + 230, sx + 50 + (i + 1) * 80 - 30, ry + 230, AMBER, 1.8, head="d"))
    b.append(arrow(sx + 105, ry + 215, sx + 105, ry + 182, AMBER, 1.8, head="d"))
    b.append(T(sx + 105, ry + 268, "swap into the same seat", 12.5, 500, MUTED, "middle"))
    b.append(T(rx + 24, ry + rh - 24, "identity in the SEAT · trajectory persists · every belief logged + gated", 13, 600, TEAL_D))
    foot = "Swap one model for a better one into the same seat and it inherits the trajectory. Identity is the seat, not the instance — like a role that outlives whoever holds it."
    return frame("The unit: a seat, not a worker",
                 "The deepest difference is what persists.",
                 foot, "\n".join(b))


# ── Slide 5 — comparison table ──────────────────────────────────────
def s5_table():
    rows = [
        ("unit", "N checkouts of ONE repo", "N independent practices"),
        ("authority", "single object store", "federated · each sovereign"),
        ("reach", "one machine (local FS)", "machines · substrates · orgs"),
        ("lifetime", "ephemeral · dies at task", "persistent seat + trajectory"),
        ("identity", "none (clones)", "in the seat · model swaps in"),
        ("carries", "code diffs", "calibrated belief + gated action"),
        ("solves", "execution isolation", "coordination + governance"),
    ]
    x0, y0, w = 80, 158, 1120
    c0, c1 = 300, 410
    c2 = w - c0 - c1
    hh, rh = 56, 56
    b = [card(x0, y0, w, hh + len(rows) * rh, "#ffffff", LINE, 1.5, 14)]
    # ours column highlight band
    b.append(f'<rect x="{x0+c0+c1}" y="{y0}" width="{c2}" height="{hh+len(rows)*rh}" rx="0" fill="{TEAL_TINT}"/>')
    # header
    b.append(f'<rect x="{x0}" y="{y0}" width="{w}" height="{hh}" rx="0" fill="{PANEL}"/>')
    b.append(f'<rect x="{x0+c0+c1}" y="{y0}" width="{c2}" height="{hh}" rx="0" fill="{TEAL}"/>')
    b.append(T(x0 + c0 + 24, y0 + 35, "worktree-style MAS", 16, 700, OLD_INK))
    b.append(T(x0 + c0 + c1 + 24, y0 + 35, "federated practices", 16, 700, "#ffffff"))
    for i, (axis, a, c) in enumerate(rows):
        ry = y0 + hh + i * rh
        if i % 2 == 1:
            b.append(f'<rect x="{x0}" y="{ry}" width="{c0+c1}" height="{rh}" fill="{PANEL}"/>')
        b.append(T(x0 + 24, ry + rh / 2 + 5, axis, 15, 700, INK))
        b.append(T(x0 + c0 + 24, ry + rh / 2 + 5, a, 14.5, 400, OLD_INK))
        b.append(T(x0 + c0 + c1 + 24, ry + rh / 2 + 5, c, 14.5, 600, TEAL_D))
    # column rules
    for cxx in (x0 + c0, x0 + c0 + c1):
        b.append(f'<line x1="{cxx}" y1="{y0}" x2="{cxx}" y2="{y0+hh+len(rows)*rh}" stroke="{LINE}" stroke-width="1"/>')
    for i in range(len(rows) + 1):
        yy = y0 + hh + i * rh
        b.append(f'<line x1="{x0}" y1="{yy}" x2="{x0+w}" y2="{yy}" stroke="{LINE}" stroke-width="1"/>')
    foot = "Worktrees parallelize one repo's branches. The mesh coordinates many sovereign agents' decisions — a generalized pull request where calibration is the review signal."
    return frame("Side by side",
                 "Same number of agents on both sides — the contrast is kind, not scale.",
                 foot, "\n".join(b))


SLIDES = [
    ("01-worktree-mas", s1_worktree),
    ("02-federated-practices", s2_federated),
    ("03-trust-rings", s3_rings),
    ("04-seat-vs-worker", s4_lifetime),
    ("05-comparison", s5_table),
]


def main():
    OUT.mkdir(exist_ok=True)
    cards = []
    for slug, fn in SLIDES:
        svg = fn()
        (OUT / f"{slug}.svg").write_text(svg, encoding="utf-8")
        cards.append(f'<section>{svg}</section>')
        print(f"wrote deck/{slug}.svg ({len(svg)} bytes)")
    html = ("<!doctype html><meta charset=utf-8><title>Mesh vs worktree-MAS</title>"
            "<style>body{background:#e2e8f0;margin:0;padding:2rem;font-family:system-ui}"
            "section{max-width:1100px;margin:0 auto 2rem;box-shadow:0 6px 24px #0003;border-radius:8px;overflow:hidden}"
            "svg{display:block;width:100%;height:auto}</style>" + "\n".join(cards))
    (OUT / "index.html").write_text(html, encoding="utf-8")
    print(f"wrote deck/index.html -> open to review")


if __name__ == "__main__":
    main()
