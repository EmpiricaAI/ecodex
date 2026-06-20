"""Pytest config for the vendored-hook Python tests.

The hook scripts under ``assets/hooks_scripts/`` are vendored verbatim from
empirica, but they RUN inside ecodex (via the Rust translation layer), so
verifying their behaviour is ecodex's responsibility. These tests exercise the
pure resolution logic without spinning up a real session.

Path setup so the hyphenated hook modules import cleanly:
  - ``assets/hooks_scripts/lib``   → bare ``import project_resolver``
  - ``assets/hooks_scripts/hooks`` → sibling hook imports
  - ``~/empirical-ai/empirica``    → ``empirica.utils.session_resolver`` (the
    same path the hooks themselves insert at runtime)
"""
from __future__ import annotations

import sys
from pathlib import Path

_PLUGIN = Path(__file__).resolve().parents[2]            # codex-empirica-plugin/
_HOOKS = _PLUGIN / "assets" / "hooks_scripts" / "hooks"
_LIB = _PLUGIN / "assets" / "hooks_scripts" / "lib"
_EMPIRICA = Path.home() / "empirical-ai" / "empirica"

for _p in (str(_LIB), str(_HOOKS), str(_EMPIRICA)):
    if _p not in sys.path:
        sys.path.insert(0, _p)
