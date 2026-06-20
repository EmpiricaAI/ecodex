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

Requires `pytest` and empirica core importable at `~/empirical-ai/empirica`
(the same path the hooks insert at runtime). If empirica isn't importable the
suite **skips** rather than fails (see `importorskip` in the test module).

## Coverage

- `test_hook_ai_id_genericization.py` — mirrors empirica's 8-test suite for the
  ai_id genericization (empirica `bb8789c74`): load-bearing defaults resolve via
  `InstanceResolver.ai_id()` with `'claude-code'` only as fallback, and the
  back-compat match-lists prepend the resolved id **only when truthy** (a leading
  `None` would short-circuit `latest_session_id` to the wildcard — the silent
  regression this guards).

## Adding coverage

When a future empirica→ecodex hook re-sync changes behaviour, add a test here
that pins the new behaviour against the vendored copy. The import helper
(`_load_hook`) handles the hyphenated filenames via `importlib`.
