#!/usr/bin/env python3
"""Generate 36 frames of the Empirica koru-spiral animation.

The Empirica logo is a koru — the unfurling-fern spiral, a Maori symbol of
new beginnings and growth. The TUI rendering captures three motifs:

  1. The almond/eye-shaped outer shell, lopsided so the tail points down-right.
  2. The counterclockwise spiral inside the shell, slowly rotating so the eye
     reads as "alive" without being distracting (~3s loop at 80ms/frame).
  3. The feather/pinion rays cascading down the left flank — three radiating
     wedges that suggest motion + echo the koru's growth direction.

Density legend (matches the upstream `frames/codex` palette):
    █  outer shell stroke + spiral core
    ▓  spiral arm, mid stroke
    ▒  spiral arm, faint stroke
    ░  ray accents

Canvas: 16 rows x 40 cols (matches existing variants).
"""

from __future__ import annotations

import math
from pathlib import Path

ROWS = 16
COLS = 40
FRAMES = 36
OUT_DIR = Path(__file__).parent

# Geometry tuned against the source PNG. The eye shape is wider above the
# midline; the tail tapers into the bottom-right quadrant.
CENTER_X = 21.0
CENTER_Y = 7.5
SHELL_RX = 13.5
SHELL_RY = 7.0
TAIL_TILT = 0.18  # clockwise tilt so the tail aims down-right
SPIRAL_TURNS = 2.7
SPIRAL_MAX_R = 6.5
SPIRAL_MIN_R = 0.4
RAY_BASE_X = 7.5
RAY_BASE_Y = 8.0


def _blank_grid() -> list[list[str]]:
    return [[" " for _ in range(COLS)] for _ in range(ROWS)]


# Stroke priority: stronger glyphs win over weaker ones at the same cell so we
# never over-draw a dense stroke with a fainter one.
_PRIORITY = "█▓▒░ "


def _put(grid: list[list[str]], x: float, y: float, ch: str) -> None:
    cx, cy = int(round(x)), int(round(y))
    if not (0 <= cx < COLS and 0 <= cy < ROWS):
        return
    cur = grid[cy][cx]
    if _PRIORITY.index(ch) <= _PRIORITY.index(cur):
        grid[cy][cx] = ch


def _draw_shell(grid: list[list[str]]) -> None:
    cos_t, sin_t = math.cos(TAIL_TILT), math.sin(TAIL_TILT)
    for step in range(0, 720):  # half-degree resolution to avoid gaps
        a = math.radians(step / 2.0)
        # Asymmetric eye: pinch the lower-right quadrant into a tail,
        # fatten the upper bulge.
        if math.pi * 0.45 < a < math.pi * 1.05:
            pinch = 0.85  # tail side
        elif math.pi * 1.55 < a < math.pi * 2.0:
            pinch = 1.05  # upper-right bulge
        else:
            pinch = 1.0
        rx = SHELL_RX * pinch
        ry = SHELL_RY * pinch
        x0 = rx * math.cos(a)
        y0 = ry * math.sin(a)
        x = CENTER_X + x0 * cos_t - y0 * sin_t
        y = CENTER_Y + x0 * sin_t + y0 * cos_t
        _put(grid, x, y, "█")


def _draw_spiral(grid: list[list[str]], phase: float) -> None:
    samples = 360
    for i in range(samples):
        t = i / samples
        # Counterclockwise rotation; phase moves the whole spiral around its
        # center so successive frames look like the spiral is breathing.
        a = -SPIRAL_TURNS * 2 * math.pi * t + phase
        r = SPIRAL_MIN_R + (SPIRAL_MAX_R - SPIRAL_MIN_R) * t
        # Terminal cells are roughly 2:1 (taller than wide); squash y to keep
        # the spiral circular looking.
        x = CENTER_X + r * 1.10 * math.cos(a)
        y = CENTER_Y + r * 0.55 * math.sin(a)
        # Stroke density: thickest at the center (the koru's eye), fading out.
        if t < 0.18:
            ch = "█"
        elif t < 0.55:
            ch = "▓"
        else:
            ch = "▒"
        _put(grid, x, y, ch)


def _draw_rays(grid: list[list[str]], phase: float) -> None:
    """Three feather-ray wedges fanning down-left from the upper-left flank.
    A small phase-driven shimmer makes them appear to pulse subtly."""
    shimmer = 0.35 * math.sin(phase * 2.0)
    rays = [
        # (start_angle, end_angle, base_r, tip_r, char)
        (math.radians(168), math.radians(192), 1.5, 7.0, "▓"),
        (math.radians(196), math.radians(218), 2.5, 8.5, "▒"),
        (math.radians(222), math.radians(245), 2.5, 7.5, "░"),
    ]
    for start, end, base_r, tip_r, ch in rays:
        for s in range(9):
            frac = s / 8.0
            a = start + (end - start) * frac
            for k in range(8):
                kfrac = k / 7.0
                r = base_r + (tip_r - base_r) * kfrac + shimmer * kfrac
                x = RAY_BASE_X + r * math.cos(a)
                y = RAY_BASE_Y + r * math.sin(a)
                _put(grid, x, y, ch)


def _frame(idx: int) -> str:
    grid = _blank_grid()
    phase = (idx / FRAMES) * 2 * math.pi
    _draw_rays(grid, phase)
    _draw_shell(grid)
    _draw_spiral(grid, phase)
    return "\n".join("".join(row).rstrip() for row in grid) + "\n"


def main() -> None:
    for i in range(1, FRAMES + 1):
        text = _frame(i - 1)
        (OUT_DIR / f"frame_{i}.txt").write_text(text)
    print(f"Wrote {FRAMES} frames to {OUT_DIR}")


if __name__ == "__main__":
    main()
