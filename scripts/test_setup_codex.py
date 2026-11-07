"""setup-codex.py: a vendored hook that imports a new hooks/lib module must get it.

Run: python3 -m pytest scripts/test_setup_codex.py -q
"""

import importlib.util
import sys
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent / "setup-codex.py"


def load():
    spec = importlib.util.spec_from_file_location("setup_codex", SCRIPT)
    assert spec is not None and spec.loader is not None
    mod = importlib.util.module_from_spec(spec)
    sys.modules["setup_codex"] = mod  # @dataclass looks the module up by name
    spec.loader.exec_module(mod)
    return mod


mod = load()


def needed(hooks, upstream, vendored, lib_sources):
    return mod.missing_lib_imports(hooks, upstream, vendored, lib_sources.__getitem__)


def test_new_lib_module_imported_by_a_hook_is_wanted():
    hooks = {"a.py": "from hook_vintage import stamp\n"}
    assert needed(hooks, {"hook_vintage.py", "other.py"}, set(), {"hook_vintage.py": ""}) == [
        "hook_vintage.py"
    ]


def test_import_inside_a_function_counts():
    hooks = {"a.py": "def f():\n    from instance_clash import x\n"}
    assert needed(hooks, {"instance_clash.py"}, set(), {"instance_clash.py": ""}) == [
        "instance_clash.py"
    ]


def test_plain_import_statement_counts():
    hooks = {"a.py": "import project_resolver\n"}
    assert needed(hooks, {"project_resolver.py"}, set(), {"project_resolver.py": ""}) == [
        "project_resolver.py"
    ]


def test_already_vendored_module_is_not_wanted_again():
    hooks = {"a.py": "from project_resolver import x\n"}
    assert needed(hooks, {"project_resolver.py"}, {"project_resolver.py"}, {}) == []


def test_unused_upstream_module_is_not_wanted():
    hooks = {"a.py": "import os\n"}
    assert needed(hooks, {"calibration_block.py"}, set(), {}) == []


def test_retired_module_is_never_wanted(monkeypatch):
    monkeypatch.setattr(mod, "RETIRED", {"first.py"})
    hooks = {"a.py": "import first\n"}
    assert needed(hooks, {"first.py"}, set(), {"first.py": ""}) == []


def test_lib_module_pulled_in_by_another_new_lib_module():
    hooks = {"a.py": "from first import x\n"}
    lib = {"first.py": "from second import y\n", "second.py": ""}
    assert needed(hooks, {"first.py", "second.py"}, set(), lib) == ["first.py", "second.py"]


def test_syntax_error_in_a_hook_does_not_stop_the_scan():
    hooks = {"bad.py": "def (:\n", "a.py": "import first\n"}
    assert needed(hooks, {"first.py"}, set(), {"first.py": ""}) == ["first.py"]


def test_import_cycle_terminates():
    hooks = {"a.py": "import first\n"}
    lib = {"first.py": "import second\n", "second.py": "import first\n"}
    assert needed(hooks, {"first.py", "second.py"}, set(), lib) == ["first.py", "second.py"]
