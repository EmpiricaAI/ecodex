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
   concatenation and name refs) remain outside static validation and are
   reported as a limitation of this guard.
3. Validates each static DML query against the real schema using SQLite's own
   parser via ``EXPLAIN`` (no third-party SQL parser).
4. Asserts no query references a missing column or table.

Empirica core is the schema instrument. If it cannot be imported, this guard
fails rather than returning a green verdict without a schema.

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
    source = py_file.read_text(encoding="utf-8")
    tree = ast.parse(source, filename=str(py_file))

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


def _created_tables(sql: str) -> set[str]:
    """Extract tables created by static DDL in the vendored hook layer."""
    import re

    pattern = re.compile(
        r"\bcreate\s+table\s+(?:if\s+not\s+exists\s+)?"
        r"([A-Za-z_][A-Za-z0-9_]*)",
        re.IGNORECASE,
    )
    return {match.group(1).lower() for match in pattern.finditer(sql)}


def _looks_like_missing_ref(message: str) -> bool:
    """True if the error names a missing column/table — the bug class."""
    msg = message.lower()
    return "no such column" in msg or "no such table" in msg or "has no column named" in msg


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
_KNOWN_VIOLATIONS: frozenset[tuple[str, str]] = frozenset(
    {
        # Upstream Empirica removed this table in favor of reflexes, but its
        # vendored tool-router still queries it. Kept visible here until fixed
        # upstream and re-vendored; unknown tables are no longer skipped.
        ("hooks/tool-router.py", "epistemic_assessments"),
    }
)


# --------------------------------------------------------------------------- #
# The test.
# --------------------------------------------------------------------------- #
def _classify_query(
    conn: sqlite3.Connection,
    relpath: str,
    sql: str,
    known_tables: set[str],
) -> tuple[str, str | None]:
    """Classify one static query against the built schema.

    Returns ``(category, message)``. category is one of: ``non_dml`` /
    ``unknown_table`` / ``validated`` (message None), or ``new`` / ``known`` /
    ``ambiguous`` (message = the OperationalError/Warning text). Extracted from
    the test body so neither function exceeds the cyclomatic-complexity budget.
    """
    keyword = _statement_keyword(sql)
    if keyword.startswith(NON_DML_PREFIXES) or keyword == "":
        return ("non_dml", None)

    tables = _primary_tables(sql)
    missing_tables = tables - known_tables
    if missing_tables:
        missing = sorted(missing_tables)[0]
        message = f"no such table: {missing}"
        key = (relpath, missing)
        return ("known" if key in _KNOWN_VIOLATIONS else "new", message)

    n_params = sql.count("?")
    try:
        conn.execute("EXPLAIN " + sql, [None] * n_params)
        return ("validated", None)
    except sqlite3.OperationalError as exc:
        message = str(exc)
        if not _looks_like_missing_ref(message):
            return ("ambiguous", message)
        key = (relpath, _missing_symbol(message))
        return ("known" if key in _KNOWN_VIOLATIONS else "new", message)
    except sqlite3.Warning as exc:
        return ("ambiguous", str(exc))


def _build_audit_report(
    n_tables: int,
    n_queries: int,
    counts: dict[str, int],
    ambiguous: list[tuple[str, str, str]],
    known_violations: list[tuple[str, str, str]],
    new_violations: list[tuple[str, str, str]],
) -> str:
    """Render the always-printed coverage report (no control-flow in the test)."""
    lines = [
        "",
        "=== vendored-hook SQL schema-reference audit ===",
        f"schema tables built          : {n_tables}",
        f"static queries extracted     : {n_queries}",
        f"  validated via EXPLAIN      : {counts['validated']}",
        f"  skipped (non-DML / DDL)    : {counts['non_dml']}",
        f"  skipped (unknown table)    : {counts['unknown_table']}",
        f"  ambiguous OperationalError : {len(ambiguous)}",
        f"  known violations (tracked) : {len(known_violations)}",
        f"  NEW violations             : {len(new_violations)}",
    ]
    if known_violations:
        lines.append("")
        lines.append("--- known violations (allow-listed, tracked for fix) ---")
        lines += [f"  {loc} — {err} — {sql.strip()[:120]}" for loc, sql, err in known_violations]
    if new_violations:
        lines.append("")
        lines.append("--- NEW VIOLATIONS (failing) ---")
        lines += [f"  {loc} — {err} — {sql.strip()[:120]}" for loc, sql, err in new_violations]
    return "\n".join(lines)


def test_static_sql_references_exist_in_schema():
    try:
        conn = _build_schema_connection()
    except ImportError as exc:
        pytest.fail(
            f"empirica core not importable ({exc}); SQL schema references were not measured"
        )

    schema = _introspect_columns(conn)
    known_tables = set(schema.keys())
    all_queries = _collect_all_static_queries()
    for _path, _line, sql in all_queries:
        if _created_tables(sql):
            conn.execute(sql)
    known_tables = set(_introspect_columns(conn))

    counts = {"validated": 0, "non_dml": 0, "unknown_table": 0}
    new_violations: list[tuple[str, str, str]] = []  # (loc, sql, error) — fail
    known_violations: list[tuple[str, str, str]] = []  # allow-listed, tracked
    ambiguous: list[tuple[str, str, str]] = []  # other OperationalErrors
    buckets = {"new": new_violations, "known": known_violations, "ambiguous": ambiguous}

    for py_file, lineno, sql in all_queries:
        relpath = py_file.relative_to(HOOKS_SCRIPTS).as_posix()
        category, message = _classify_query(conn, relpath, sql, known_tables)
        if category in counts:
            counts[category] += 1
        else:
            # new/known/ambiguous always carry a message; `or ""` keeps the
            # tuple typed str (classifier returns str | None for the count cases).
            buckets[category].append((f"{relpath}:{lineno}", sql, message or ""))

    conn.close()

    print(
        _build_audit_report(
            len(known_tables),
            len(all_queries),
            counts,
            ambiguous,
            known_violations,
            new_violations,
        )
    )

    assert len(known_violations) == len(_KNOWN_VIOLATIONS), (
        "the known SQL violation ratchet is stale or did not observe every "
        f"entry: expected {_KNOWN_VIOLATIONS}, observed {known_violations}"
    )

    assert not new_violations, (
        f"{len(new_violations)} NEW static SQL quer"
        f"{'y' if len(new_violations) == 1 else 'ies'} in the vendored hooks "
        f"reference a column/table that does not exist in empirica's schema "
        f"(the bug class this guard catches):\n"
        + "\n".join(f"  {loc} — {err} — {sql.strip()[:120]}" for loc, sql, err in new_violations)
        + "\n\nFix the query (use the real column), re-vendor the hook from a "
        "matching empirica ref, OR — only if it's genuinely a pre-existing "
        "tracked case — add (relpath, symbol) to _KNOWN_VIOLATIONS with "
        "justification."
    )


if __name__ == "__main__":
    pytest.main([__file__, "-q", "-s"])
