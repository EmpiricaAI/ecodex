# Vendored-hook tests

Python tests for the empirica hook scripts vendored under
`../../assets/hooks_scripts/`. The scripts are synced verbatim from empirica,
but they execute inside ecodex (via the Rust translation layer), so their
behaviour is ecodex's to verify — `py_compile` + diff-against-upstream isn't
enough on its own.

## Run

```bash
# from repo root
scripts/test-vendored-hooks.sh
# or directly
python3 -m pytest codex-rs/codex-empirica-plugin/tests/vendored_hooks/ -v
```

Requires `pytest` and the matching Empirica core revision. `conftest.py`
supplies a narrow `InstanceResolver` stub so tests that do not need the real
core remain hermetic, but parser/schema guards fail if their production
instrument is unavailable. CI checks out and installs the pinned matching
Empirica revision before running this suite.

## Coverage

- `test_hook_ai_id_genericization.py` — mirrors empirica's 8-test suite for the
  ai_id genericization (empirica `bb8789c74`): load-bearing defaults resolve via
  `InstanceResolver.ai_id()` with `'claude-code'` only as fallback, and the
  back-compat match-lists prepend the resolved id **only when truthy** (a leading
  `None` would short-circuit `latest_session_id` to the wildcard — the silent
  regression this guards).
- `test_sql_schema_references.py` — adapted port of empirica's SQL schema-ref
  guard (empirica `168fd1041`). AST-extracts every *static* SQL string the
  vendored hooks pass to `.execute()`/`.executemany()`/`.executescript()` and
  validates each against empirica's **real** schema (built in-memory from the
  production schema builders) via SQLite's own `EXPLAIN` parser. Fails on any
  query referencing a missing column/table — the silent-no-op bug class that the
  `created_timestamp`/`epistemic_importance` drift (fixed in `60a8c5b35e`) fell
  into. Hook-created static tables are installed into the test schema before
  validation, and the one upstream-removed table reference remains an explicit
  known violation until it is fixed upstream and re-vendored.
- `test_import_budget.py` — adapted port of empirica's import-budget gate
  (empirica `d1f5dc736`). The Rust layer spawns a vendored hook as a fresh
  subprocess on every hot event (sentinel-gate on every Bash/Edit/Write,
  tool-router on every prompt, …), so a heavy import at a hook's module top-level
  taxes every spawn. Loads each hot-path hook by path in a fresh subprocess and
  asserts none of `_HEAVY` (LLM/embedding SDKs, httpx, GitPython, qdrant, ML/data
  libs, fastapi) landed in `sys.modules`. Presence-based (deterministic), not
  time-based (flaky). `_BUDGET` per-hook allow-sets are empty — baseline confirms
  every critical hook pulls empirica core lazily and nothing heavy.
- `test_cli_parity.py` — source-discovers every concrete `empirica` subprocess
  argv in the vendored Python hooks and Rust core, then checks each nested
  command and referenced flag against Empirica's real argparse parser. This
  catches wrapper/CLI flag drift without maintaining a second command map.

## Adding coverage

When a future empirica→ecodex hook re-sync changes behaviour, add a test here
that pins the new behaviour against the vendored copy. The import helper
(`_load_hook`) handles the hyphenated filenames via `importlib`.
