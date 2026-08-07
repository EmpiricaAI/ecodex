"""Pytest config for the vendored-hook Python tests.

The hook scripts under ``assets/hooks_scripts/`` are vendored verbatim from
empirica, but they RUN inside ecodex (via the Rust translation layer), so
verifying their behaviour is ecodex's responsibility. These tests exercise the
pure resolution logic without spinning up a real session.

Path setup so the hyphenated hook modules import cleanly:
  - ``assets/hooks_scripts/lib``   → bare ``import project_resolver``
  - ``assets/hooks_scripts/hooks`` → sibling hook imports

When empirica core is not installed, a meta-path finder supplies a minimal
``empirica.*`` hierarchy for hook imports. A real installation always wins.
"""
from __future__ import annotations

import importlib.util
import sys
from importlib.abc import Loader, MetaPathFinder
from importlib.machinery import ModuleSpec
from pathlib import Path
from types import ModuleType

_PLUGIN = Path(__file__).resolve().parents[2]            # codex-empirica-plugin/
_HOOKS = _PLUGIN / "assets" / "hooks_scripts" / "hooks"
_LIB = _PLUGIN / "assets" / "hooks_scripts" / "lib"

for _p in (str(_LIB), str(_HOOKS)):
    if _p not in sys.path:
        sys.path.insert(0, _p)


class StubInstanceResolver:
    """Controllable subset of the production resolver used by these tests."""

    @staticmethod
    def ai_id(
        claude_session_id: str | None = None,
        project_path: str | Path | None = None,
    ) -> str | None:
        return None

    @staticmethod
    def latest_session_id(
        ai_id: str | None = None,
        active_only: bool = False,
    ) -> str | None:
        return None


class _EmpiricaStubFinder(MetaPathFinder, Loader):
    """Fabricate empirica modules lazily when core is genuinely absent."""

    _SESSION_RESOLVER = "empirica.utils.session_resolver"
    _PACKAGES = {"empirica", "empirica.utils"}

    def find_spec(
        self,
        fullname: str,
        path: object = None,
        target: ModuleType | None = None,
    ) -> ModuleSpec | None:
        if fullname == "empirica" or fullname.startswith("empirica."):
            return ModuleSpec(fullname, self, is_package=fullname in self._PACKAGES)
        return None

    def create_module(self, spec: ModuleSpec) -> ModuleType | None:
        return None

    def exec_module(self, module: ModuleType) -> None:
        if module.__name__ == self._SESSION_RESOLVER:
            module.InstanceResolver = StubInstanceResolver


if importlib.util.find_spec("empirica") is None:
    sys.meta_path.insert(0, _EmpiricaStubFinder())
