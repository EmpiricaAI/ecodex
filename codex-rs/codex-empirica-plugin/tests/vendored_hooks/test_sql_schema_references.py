"""Catch a recurring bug class in the VENDORED hooks: static SQL that references
a non-existent column/table in empirica's real schema.

Adapted from empirica's tests/test_sql_schema_references.py (@168fd1041, proposal
prop_xhqjdfeafbhutphxagv3pz4h5q). The transferable invariant is unchanged —
"validate every static query string against the real schema at test time" — but
the scan target and skip behaviour are ecodex-specific.

THE BUG CLASS
-------------
A static SQL query names a column or table that does not exist in the real
schema. At runtime SQLite raises ``OperationalError: no such column`` (or
``no such table``), but the call site wraps the query in a broad ``try/except``
that swallows the error. The feature then silently no-ops while every health
surface looks green.

This is not hypothetical for ecodex: the vendored hooks carried two LIVE
instances of exactly this — ``timestamp`` → ``created_timestamp`` in
sentinel-gate, ``importance`` → ``epistemic_importance`` in post-compact — both
fixed manually in commit 60a8c5b35e *after* they had silently no-op'd for an
unknown duration. This guard retro-catches that class and fails CI on any new
occurrence introduced by a re-vendor or an empirica schema change.

WHY THE SCHEMA BUILD IS UNCHANGED FROM EMPIRICA
-----------------------------------------------
The vendored hooks under ``assets/hooks_scripts/`` read/write empirica's OWN
session + workspace databases (shared via the installed empirica core, not a
separate store). So the *real* schema for them IS empirica's schema — we build
it from empirica's production schema builders exactly as empirica's own guard
does. No parser swap, no hand-mirrored copy.

WHAT THIS TEST DOES
-------------------
1. Builds the real schema (session DB + workspace DB) in an in-memory SQLite
   connection via empirica's production schema builders.
2. AST-walks every ``.py`` under ``assets/hooks_scripts/`` and extracts every
   *static* SQL string passed to ``.execute()`` / ``.executemany()`` /
   ``.executescript()``. Dynamic queries (f-strings, ``.format()``, ``%``,
   concatenation, name refs) are SKIPPED on purpose — they interpolate
   identifiers from internal allow-lists (known-correct, unresolvable
   statically).
3. Validates each static DML query against the real schema using SQLite's own
   parser via ``EXPLAIN`` (no third-party SQL parser).
4. Asserts no query references a missing column or table.

SKIP, DON'T FAIL, WHEN EMPIRICA CORE IS ABSENT
----------------------------------------------
Unlike empirica's in-repo guard, this test runs in ecodex where empirica core
is a dev-checkout dependency on ``sys.path`` (set by conftest.py). If it can't
be imported (e.g. hermetic CI without the checkout) the test ``pytest.skip``s
rather than failing — consistent with scripts/test-vendored-hooks.sh's
"skip if empirica core unavailable" philosophy and the logged hermetic-CI goal.

KNOWN LIMITATION — migration drift
----------------------------------
Builds the *fresh* schema (all CREATE TABLE + migrations). Catches columns
missing from the fresh schema, but NOT the migration-drift variant (a column
present in the fresh schema but absent from an older long-lived DB). Same scope
boundary as empirica's guard.
"""

from __future__ import annotations

import ast
import sqlite3
import tempfile
from pathlib import Path

import pytest

# This test file lives at tests/vendored_hooks/ inside the plugin crate.
# parents[2] == codex-empirica-plugin/ (same anchor conftest.py uses).
PLUGIN_ROOT = Path(__file__).resolve().parents[2]
HOOKS_SCRIPTS = PLUGIN_ROOT / "assets" / "hooks_scripts"

# Subdirs under hooks_scripts/ we never scan.
SKIP_DIR_NAMES = {"tests", "build", "dev_scripts", "__pycache__", "migrations"}

# Statement leading keywords we do NOT validate (DDL / pragmas / control).
NON_DML_PREFIXES = (
    "create",
    "alter",
    "drop",
    "pragma",
    "explain",
    "attach",
    "detach",
    "begin",
    "commit",
    "rollback",
    "savepoint",
    "release",
    "vacuum",
    "analyze",
    "reindex",
    "with",  # CTEs — EXPLAIN of a bare CTE is brittle; skip to avoid noise.
)

# Methods whose first positional arg is a SQL string.
SQL_EXEC_METHODS = {"execute", "executemany", "executescript"}


# --------------------------------------------------------------------------- #
# Step 1 — build the union schema in one in-memory connection.
# --------------------------------------------------------------------------- #
def _build_schema_connection() -> sqlite3.Connection:
    """Build session + workspace schema in one connection; return it live.

    Imports empirica's production schema builders (on sys.path via conftest).
    Raises ImportError if empirica core is absent — the caller turns that into
    a pytest.skip.
    """
    from empirica.data.repositories.workspace_db import _ensure_workspace_schema
    from empirica.data.session_database import SessionDatabase

    tmpdir = tempfile.mkdtemp(prefix="ecodex_sql_schema_test_")
    db_path = str(Path(tmpdir) / "sessions.db")

    sdb = SessionDatabase(db_path=db_path, db_type="sqlite")
    conn = sdb.conn
    assert conn is not None, "SessionDatabase did not open a connection"

    # Union the workspace tables (global_projects, etc.) onto the SAME conn.
    # CREATE TABLE IF NOT EXISTS semantics make duplicate names harmless.
    _ensure_workspace_schema(conn)

    return conn


def _introspect_columns(conn: sqlite3.Connection) -> dict[str, set[str]]:
    """Return ``{table_name: set(column_names)}`` for every table/view."""
    cur = conn.cursor()
    cur.execute("SELECT name FROM sqlite_master WHERE type IN ('table', 'view')")
    tables = [r[0] for r in cur.fetchall()]
    schema: dict[str, set[str]] = {}
    for table in tables:
        cur.execute(f"PRAGMA table_info({table})")
        schema[table] = {row[1] for row in cur.fetchall()}
    return schema


# --------------------------------------------------------------------------- #
# Step 2 — extract static SQL queries via AST.
# --------------------------------------------------------------------------- #
def _const_str(node: ast.AST) -> str | None:
    """Return the string value if ``node`` is a static string literal.

    Returns None for anything dynamic: f-strings (``JoinedStr``), names,
    attributes, calls (``.format()``), binary ops (``%`` / ``+``).
    """
    if isinstance(node, ast.Constant) and isinstance(node.value, str):
        return node.value
    return None


def _iter_static_queries(py_file: Path) -> list[tuple[int, str]]:
    """Yield ``(lineno, sql)`` for each static SQL exec call in ``py_file``."""
    try:
        source = py_file.read_text(encoding="utf-8")
    except (UnicodeDecodeError, OSError):
        return []
    try:
        tree = ast.parse(source, filename=str(py_file))
    except SyntaxError:
        return []

    found: list[tuple[int, str]] = []
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        func = node.func
        if not isinstance(func, ast.Attribute):
            continue
        if func.attr not in SQL_EXEC_METHODS:
            continue
        if not node.args:
            continue
        sql = _const_str(node.args[0])
        if sql is None:
            continue  # dynamic query — intentionally skipped
        found.append((node.lineno, sql))
    return found


def _collect_all_static_queries() -> list[tuple[Path, int, str]]:
    """Walk ``assets/hooks_scripts/`` and collect every static SQL query."""
    queries: list[tuple[Path, int, str]] = []
    for py_file in HOOKS_SCRIPTS.rglob("*.py"):
        if any(part in SKIP_DIR_NAMES for part in py_file.relative_to(HOOKS_SCRIPTS).parts):
            continue
        for lineno, sql in _iter_static_queries(py_file):
            queries.append((py_file, lineno, sql))
    return queries


# --------------------------------------------------------------------------- #
# Step 3 — validate a single query against the built schema.
# --------------------------------------------------------------------------- #
def _statement_keyword(sql: str) -> str:
    """Return the leading SQL keyword, lowercased (after stripping comments)."""
    stripped = sql.lstrip()
    while stripped.startswith("--"):
        nl = stripped.find("\n")
        if nl == -1:
            return ""
        stripped = stripped[nl + 1 :].lstrip()
    if not stripped:
        return ""
    return stripped.split(None, 1)[0].lower()


def _primary_tables(sql: str) -> set[str]:
    """Heuristically extract referenced table names (FROM/JOIN/UPDATE/INTO)."""
    import re

    tables: set[str] = set()
    pattern = re.compile(
        r"\b(?:from|join|into|update)\s+([A-Za-z_][A-Za-z0-9_]*)",
        re.IGNORECASE,
    )
    for match in pattern.finditer(sql):
        tables.add(match.group(1).lower())
    return tables


def _looks_like_missing_ref(message: str) -> bool:
    """True if the error names a missing column/table — the bug class."""
    msg = message.lower()
    return (
        "no such column" in msg
        or "no such table" in msg
        or "has no column named" in msg
    )


def _missing_symbol(message: str) -> str:
    """Extract the offending column/table name from a missing-ref error."""
    import re

    for pat in (
        r"no such column:\s*([A-Za-z_][\w.]*)",
        r"no such table:\s*([A-Za-z_][\w.]*)",
        r"has no column named\s+([A-Za-z_][\w.]*)",
    ):
        m = re.search(pat, message, re.IGNORECASE)
        if m:
            return m.group(1).split(".")[-1].lower()
    return ""


# Ratchet allow-list of PRE-EXISTING violations surfaced by the first audit run.
# Keyed by (posix-relpath-under-hooks_scripts, missing-symbol) so it's stable
# across line shifts. The test FAILS on any NEW violation not in this set; as
# each known bug is fixed its entry is removed (a ratchet, not a sweep).
#
# Entries here are real, separately-tracked bugs — fix-tracking goal is logged
# in empirica. They are listed in the open, CI-guarded against regrowth.
_KNOWN_VIOLATIONS: frozenset[tuple[str, str]] = frozenset()


# --------------------------------------------------------------------------- #
# The test.
# --------------------------------------------------------------------------- #
def test_static_sql_references_exist_in_schema():
    try:
        conn = _build_schema_connection()
    except ImportError as exc:
        pytest.skip(
            f"empirica core not importable ({exc}); SQL schema-ref guard needs "
            "the empirica dev checkout on sys.path (see conftest.py / "
            "test-vendored-hooks.sh)."
        )

    schema = _introspect_columns(conn)
    known_tables = set(schema.keys())

    all_queries = _collect_all_static_queries()

    validated = 0
    skipped_non_dml = 0
    skipped_unknown_table = 0
    new_violations: list[tuple[str, str, str]] = []    # (loc, sql, error) — fail
    known_violations: list[tuple[str, str, str]] = []  # allow-listed, tracked
    ambiguous: list[tuple[str, str, str]] = []  # other OperationalErrors

    for py_file, lineno, sql in all_queries:
        relpath = py_file.relative_to(HOOKS_SCRIPTS).as_posix()
        loc = f"{relpath}:{lineno}"

        keyword = _statement_keyword(sql)
        if keyword.startswith(NON_DML_PREFIXES) or keyword == "":
            skipped_non_dml += 1
            continue

        tables = _primary_tables(sql)
        if tables and not (tables & known_tables):
            skipped_unknown_table += 1
            continue

        n_params = sql.count("?")
        try:
            conn.execute("EXPLAIN " + sql, [None] * n_params)
            validated += 1
        except sqlite3.OperationalError as exc:
            message = str(exc)
            if _looks_like_missing_ref(message):
                key = (relpath, _missing_symbol(message))
                if key in _KNOWN_VIOLATIONS:
                    known_violations.append((loc, sql, message))
                else:
                    new_violations.append((loc, sql, message))
            else:
                ambiguous.append((loc, sql, message))
        except sqlite3.Warning as exc:
            ambiguous.append((loc, sql, str(exc)))

    report_lines = [
        "",
        "=== vendored-hook SQL schema-reference audit ===",
        f"schema tables built          : {len(known_tables)}",
        f"static queries extracted     : {len(all_queries)}",
        f"  validated via EXPLAIN      : {validated}",
        f"  skipped (non-DML / DDL)    : {skipped_non_dml}",
        f"  skipped (unknown table)    : {skipped_unknown_table}",
        f"  ambiguous OperationalError : {len(ambiguous)}",
        f"  known violations (tracked) : {len(known_violations)}",
        f"  NEW violations             : {len(new_violations)}",
    ]
    if known_violations:
        report_lines.append("")
        report_lines.append("--- known violations (allow-listed, tracked for fix) ---")
        for loc, sql, err in known_violations:
            report_lines.append(f"  {loc} — {err} — {sql.strip()[:120]}")
    if new_violations:
        report_lines.append("")
        report_lines.append("--- NEW VIOLATIONS (failing) ---")
        for loc, sql, err in new_violations:
            report_lines.append(f"  {loc} — {err} — {sql.strip()[:120]}")
    print("\n".join(report_lines))

    conn.close()

    assert not new_violations, (
        f"{len(new_violations)} NEW static SQL quer"
        f"{'y' if len(new_violations) == 1 else 'ies'} in the vendored hooks "
        f"reference a column/table that does not exist in empirica's schema "
        f"(the bug class this guard catches):\n"
        + "\n".join(
            f"  {loc} — {err} — {sql.strip()[:120]}" for loc, sql, err in new_violations
        )
        + "\n\nFix the query (use the real column), re-vendor the hook from a "
        "matching empirica ref, OR — only if it's genuinely a pre-existing "
        "tracked case — add (relpath, symbol) to _KNOWN_VIOLATIONS with "
        "justification."
    )


if __name__ == "__main__":
    pytest.main([__file__, "-q", "-s"])
