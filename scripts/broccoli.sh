#!/usr/bin/env bash
# eat-the-broccoli 🥦 — ecodex's deterministic quality sweep (Rust stack)
#
# The automatable half of the `eat-the-broccoli` skill (Phase 1: deterministic
# tooling) wired for ecodex. Run it before a release, after a refactor, or when
# something smells off. Phase 2 (the pattern hunt — races, stale caches, silent
# fallbacks, membrane misses) is NOT automatable; load the skill for that:
#   ~/.claude/plugins/local/empirica/skills/eat-the-broccoli/SKILL.md
#
# SCOPE — this is a FORK. Most of codex-rs is upstream openai/codex code we don't
# maintain, so the GATING checks scope to the crates ecodex actually owns
# (codex-empirica-plugin, codex-empirica-translator, codex-cli) plus ecodex's own
# firewall/contract guards. Sweeping the full workspace would drown in upstream
# debt (the scope dial is load-bearing here, not optional — see the skill).
#
# GATING vs INFORMATIONAL. Gating checks fail the sweep (things we own + can fix).
# The dependency-advisory tools (cargo-audit/deny/machete/geiger) run
# INFORMATIONAL-only: they cover the SHARED upstream dependency graph, and known
# advisories are often upstream-blocked (hickory-proto, opentelemetry — tracked
# separately). Gating on them would make broccoli a check that can never pass —
# the exact "verb that always fails" anti-pattern the skill warns about. They
# print their findings so a human can triage; they never set the exit code.
#
# Usage:
#   ./scripts/broccoli.sh                 full sweep (gating + informational)
#   ./scripts/broccoli.sh --quick         gating fmt + clippy + owned tests only
#   ./scripts/broccoli.sh --no-informational   gating checks only, skip advisories
#
# Exit code: 0 iff every GATING check passed. Informational findings never fail it.

set -uo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
ECODEX_ROOT="$(cd -- "${SCRIPT_DIR}/.." &>/dev/null && pwd)"
CODEX_RS="${ECODEX_ROOT}/codex-rs"

# ecodex-owned crates for clippy/test (matches release.sh gate-test scope).
# codex-cli is upstream-derived but ecodex-patched (branding, update surfaces)
# and kept clippy/test-clean, so it's gated here.
OWNED_PKGS=(-p codex-empirica-plugin -p codex-empirica-translator -p codex-cli)

# The crates ecodex authors in FULL — used for fmt, tests, and the silent-failure
# scan, all of which examine code line-by-line rather than lint for specific
# issues. Including codex-cli here would drag in upstream noise ecodex doesn't own:
#   - fmt: upstream's nightly-rustfmt style (`imports_granularity = Item`, which
#     stable can't apply) shows as spurious diffs in untouched upstream lines.
#   - tests: codex-cli carries env/auth-sensitive tests (doctor snapshots embed
#     the live auth row; they fail on a dev box with an expired token — a
#     config/env-divergence, not a defect) that belong in CI, not a local sweep.
# clippy (below) DOES include codex-cli: it's static, auth-independent, passes,
# and ecodex's user-facing patches live there.
PURE_PKGS=(-p codex-empirica-plugin -p codex-empirica-translator)

QUICK=0
RUN_INFORMATIONAL=1
for arg in "$@"; do
  case "$arg" in
    --quick)            QUICK=1 ;;
    --no-informational) RUN_INFORMATIONAL=0 ;;
    -h|--help)          sed -n '2,40p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) echo "broccoli.sh: unknown arg '$arg' (see --help)" >&2; exit 64 ;;
  esac
done

# ── output helpers ───────────────────────────────────────────────────
bold()  { printf '\033[1m%s\033[0m\n' "$*"; }
green() { printf '\033[32m%s\033[0m\n' "$*"; }
red()   { printf '\033[31m%s\033[0m\n' "$*"; }
yellow(){ printf '\033[33m%s\033[0m\n' "$*"; }

GATING_FAILURES=()

# run_gate <label> <cmd...> — run a gating check; record failure, never exit early.
run_gate() {
  local label="$1"; shift
  bold "▶ ${label}"
  if "$@"; then
    green "  ✓ ${label}"
  else
    red   "  ✗ ${label}"
    GATING_FAILURES+=("${label}")
  fi
}

# run_info <label> <cmd...> — informational: print, never affect exit code.
run_info() {
  local label="$1"; shift
  bold "▶ (info) ${label}"
  if "$@"; then
    green "  · ${label}: clean"
  else
    yellow "  · ${label}: findings above — triage, not a gate"
  fi
}

cd "${CODEX_RS}"

bold "🥦 eat-the-broccoli — ecodex deterministic sweep (owned crates)"
echo "   scope: ${OWNED_PKGS[*]}"
echo

# ── GATING: format, lint, tests (things we own) ──────────────────────
run_gate "rustfmt --check"        cargo fmt --check "${PURE_PKGS[@]}"
run_gate "clippy -D warnings"     env RUST_MIN_STACK=16777216 cargo clippy "${OWNED_PKGS[@]}" --all-targets -- -D warnings
run_gate "owned-crate tests"      env RUST_MIN_STACK=16777216 cargo test "${PURE_PKGS[@]}"

if [[ "${QUICK}" -eq 1 ]]; then
  echo
  bold "quick mode — skipping guards, contract tests, and informational tools"
else
  # ── INFORMATIONAL: silent-failure hunt in pure-ecodex Rust src ───────
  # A grep can locate `.ok();` (Result silently dropped) and `let _ = call();`
  # (fallible result discarded with NO `?`), but it cannot judge intent — a
  # discard is often deliberate. So this REPORTS, never gates: a gating grep
  # that fires on benign discards is the "check that always fails" anti-pattern
  # the skill warns about (it trains dismissal). Note it deliberately does NOT
  # match `let _ = call()?;` — the `?` PROPAGATES the error, so only the Ok
  # value is dropped, which is not a swallow. Scoped to pure-ecodex src only.
  run_info "silent-failure scan (pure-ecodex src)" bash -c '
    hits=0
    for c in codex-empirica-plugin codex-empirica-translator; do
      d="'"${CODEX_RS}"'/$c/src"
      [ -d "$d" ] || continue
      # `.ok();` discards, and `let _ = <call>;` with no trailing `?`.
      n=$(grep -rnE "\.ok\(\)\s*;|let _ = [^;]*\)\s*;" "$d" 2>/dev/null \
            | grep -vE "\?\s*;" | grep -viE "test|#\[cfg\(test\)\]" | wc -l)
      if [ "$n" -gt 0 ]; then
        echo "  $c: $n candidate discard site(s) — review intent"
        grep -rnE "\.ok\(\)\s*;|let _ = [^;]*\)\s*;" "$d" 2>/dev/null \
          | grep -vE "\?\s*;" | grep -viE "test|#\[cfg\(test\)\]" | head -10
      fi
      hits=$((hits + n))
    done
    [ "$hits" -eq 0 ] && echo "  no silent discards in pure-ecodex src"
    exit 0
  '

  # ── GATING: ecodex contract/firewall guards ──────────────────────────
  run_gate "vendored firewall drift-guard" python3 "${ECODEX_ROOT}/scripts/check_vendored_firewall.py"
  run_gate "UPSTREAM_SYNC_TAG drift-guard"  python3 "${ECODEX_ROOT}/scripts/check_upstream_sync_tag.py"
  run_gate "Hugging Face contract"          python3 "${ECODEX_ROOT}/scripts/check_huggingface_integration.py"

  # ── GATING: hermetic vendored-hook suite (needs pytest + empirica) ───
  if python3 -c 'import pytest, empirica' >/dev/null 2>&1; then
    run_gate "vendored-hook suite" python3 -m pytest "${CODEX_RS}/codex-empirica-plugin/tests/vendored_hooks/" -q
  else
    yellow "▶ vendored-hook suite: SKIPPED — pytest or empirica not importable in this interpreter"
    yellow "  (CI runs it in a hermetic venv; not a pass, a skip — install both to gate it here)"
  fi

  # ── INFORMATIONAL: dependency advisories over the SHARED graph ───────
  if [[ "${RUN_INFORMATIONAL}" -eq 1 ]]; then
    echo
    bold "── informational (whole dep graph — upstream included; never gates) ──"
    run_info "cargo audit (CVEs)"          cargo audit
    run_info "cargo deny check"            cargo deny check
    run_info "cargo machete (unused deps)" cargo machete
  fi
fi

# ── summary ──────────────────────────────────────────────────────────
echo
if [[ "${#GATING_FAILURES[@]}" -eq 0 ]]; then
  green "🥦 broccoli: all gating checks passed."
  echo "   Phase 2 (pattern hunt — races, stale caches, silent fallbacks, membrane"
  echo "   misses) is manual: load the eat-the-broccoli skill and walk the diff."
  exit 0
else
  red "🥦 broccoli: ${#GATING_FAILURES[@]} gating check(s) failed:"
  for f in "${GATING_FAILURES[@]}"; do red "   ✗ ${f}"; done
  exit 1
fi
