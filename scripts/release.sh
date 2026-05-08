#!/usr/bin/env bash
# ecodex release pipeline
#
# Phase 1 (Tx-BC): version bump → CHANGELOG roll → commit → tag.
# Phase 2 (Tx-BD, this script): opt-in build/test/clippy gates + push + gh release.
# Phase 3 (planned, Tx-BE): cargo publish (own crates), homebrew, npm channels.
#
# Replaces the placeholder stub from Tx-AY (which exit-64'd with a "not yet
# implemented" message). Phase 1 is the deterministic, reversible part of
# the release flow — everything that doesn't touch a remote can be safely
# re-run with `git reset` if a step goes sideways.
#
# Usage:
#   ./scripts/release.sh --patch                Bump 0.1.2 → 0.1.3
#   ./scripts/release.sh --minor                Bump 0.1.2 → 0.2.0
#   ./scripts/release.sh --major                Bump 0.1.2 → 1.0.0
#   ./scripts/release.sh --explicit 0.5.0       Set version to exactly 0.5.0
#   ./scripts/release.sh --patch --dry-run      Print actions without writing
#   ./scripts/release.sh --patch --skip-tag     Bump + commit only, no tag
#   ./scripts/release.sh --patch --skip-commit  Edit files in place, leave
#                                               for manual commit + review.
#   ./scripts/release.sh --patch --gate-all     Bump + roll + run cargo
#                                               build + test + clippy as
#                                               gates BEFORE commit/tag.
#                                               Failure leaves dirty tree
#                                               for review.
#   ./scripts/release.sh --patch --gate-all --push --create-gh-release
#                                               Full Phase 1+2 flow:
#                                               bump → roll → gates →
#                                               commit → tag → push →
#                                               gh release create.
#
# Required state at invocation:
#   - working tree clean (no unstaged or uncommitted changes outside the
#     files we're about to touch)
#   - on a branch (not detached HEAD)
#   - codex-rs/Cargo.toml has [workspace.package] with `version = "X.Y.Z"`
#   - CHANGELOG.md has an `## [Unreleased]` section near the top
#
# All exit non-zero on validation failure; safe to re-run after fixing.

set -euo pipefail

# ─── Defaults ────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
ECODEX_ROOT="$(cd -- "${SCRIPT_DIR}/.." &>/dev/null && pwd)"

CARGO_TOML="${ECODEX_ROOT}/codex-rs/Cargo.toml"
CHANGELOG="${ECODEX_ROOT}/CHANGELOG.md"

BUMP_KIND=""        # major | minor | patch | explicit
EXPLICIT_VERSION=""
DRY_RUN=0
SKIP_TAG=0
SKIP_COMMIT=0
SKIP_CHANGELOG=0
GATE_BUILD=0
GATE_TEST=0
GATE_CLIPPY=0
PUSH=0
CREATE_GH_RELEASE=0

# ─── Parse args ──────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --major)         BUMP_KIND="major";         shift ;;
    --minor)         BUMP_KIND="minor";         shift ;;
    --patch)         BUMP_KIND="patch";         shift ;;
    --explicit)
      BUMP_KIND="explicit"
      EXPLICIT_VERSION="$2"
      shift 2
      ;;
    --dry-run)       DRY_RUN=1;                 shift ;;
    --skip-tag)      SKIP_TAG=1;                shift ;;
    --skip-commit)   SKIP_COMMIT=1;             shift ;;
    --skip-changelog) SKIP_CHANGELOG=1;         shift ;;
    --gate-build)    GATE_BUILD=1;              shift ;;
    --gate-test)     GATE_TEST=1;               shift ;;
    --gate-clippy)   GATE_CLIPPY=1;             shift ;;
    --gate-all)
      GATE_BUILD=1
      GATE_TEST=1
      GATE_CLIPPY=1
      shift
      ;;
    --push)             PUSH=1;                 shift ;;
    --create-gh-release) CREATE_GH_RELEASE=1;   shift ;;
    -h|--help)
      sed -n '2,30p' "${BASH_SOURCE[0]}" | sed 's/^# //; s/^#//'
      exit 0
      ;;
    *) echo "release.sh: unknown arg '$1'" >&2; exit 64 ;;
  esac
done

if [[ -z "$BUMP_KIND" ]]; then
  echo "release.sh: must supply one of --major / --minor / --patch / --explicit X.Y.Z" >&2
  echo "Run --help for full usage." >&2
  exit 64
fi

# ─── Helpers ─────────────────────────────────────────────────────────
log()   { printf '→ %s\n' "$1" >&2; }
warn()  { printf '⚠ %s\n' "$1" >&2; }
error() { printf '✗ %s\n' "$1" >&2; exit 1; }

run_or_dry() {
  if [[ "$DRY_RUN" -eq 1 ]]; then
    printf '  [dry-run] %s\n' "$*" >&2
  else
    "$@"
  fi
}

# ─── Validation ──────────────────────────────────────────────────────
[[ -f "$CARGO_TOML" ]] || error "Cargo.toml not found at $CARGO_TOML"
[[ -f "$CHANGELOG" ]]  || error "CHANGELOG.md not found at $CHANGELOG"

cd "$ECODEX_ROOT"

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  error "not inside a git working tree"
fi

current_branch="$(git rev-parse --abbrev-ref HEAD)"
if [[ "$current_branch" == "HEAD" ]]; then
  error "detached HEAD; check out a branch first"
fi

if [[ "$DRY_RUN" -eq 0 && "$SKIP_COMMIT" -eq 0 ]]; then
  if [[ -n "$(git status --porcelain)" ]]; then
    error "working tree not clean — commit or stash first, then re-run"
  fi
fi

# ─── Read current version ────────────────────────────────────────────
# Match the FIRST `version = "X.Y.Z"` after `[workspace.package]`.
# awk window keeps us in the right TOML section so we don't pick up a
# dependency's version field.
current_version="$(awk '
  /^\[workspace\.package\]/ { in_section = 1; next }
  /^\[/ { in_section = 0 }
  in_section && /^[[:space:]]*version[[:space:]]*=/ {
    match($0, /"[^"]*"/)
    if (RSTART > 0) {
      print substr($0, RSTART + 1, RLENGTH - 2)
      exit
    }
  }
' "$CARGO_TOML")"

if [[ -z "$current_version" ]]; then
  error "couldn't parse version from $CARGO_TOML [workspace.package]"
fi

# Validate semver shape (X.Y.Z, all numeric).
if [[ ! "$current_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  error "current version '$current_version' isn't a clean X.Y.Z semver"
fi

IFS='.' read -r cur_major cur_minor cur_patch <<<"$current_version"

# ─── Compute new version ─────────────────────────────────────────────
case "$BUMP_KIND" in
  major)
    new_version="$((cur_major + 1)).0.0"
    ;;
  minor)
    new_version="${cur_major}.$((cur_minor + 1)).0"
    ;;
  patch)
    new_version="${cur_major}.${cur_minor}.$((cur_patch + 1))"
    ;;
  explicit)
    if [[ ! "$EXPLICIT_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
      error "--explicit version '$EXPLICIT_VERSION' isn't a clean X.Y.Z semver"
    fi
    new_version="$EXPLICIT_VERSION"
    ;;
esac

if [[ "$new_version" == "$current_version" ]]; then
  error "computed version $new_version equals current — nothing to release"
fi

log "Current version: $current_version"
log "New version:     $new_version  ($BUMP_KIND)"

# ─── Update Cargo.toml ───────────────────────────────────────────────
# In-place sed targeting the workspace.package version line. We anchor
# on the section header to avoid mutating dependency versions. awk
# would be more robust against future structure shifts, but the sed
# form is shorter and easier to verify in `git diff` against the
# expected one-line change.
log "Updating $CARGO_TOML"
if [[ "$DRY_RUN" -eq 0 ]]; then
  python3 - "$CARGO_TOML" "$current_version" "$new_version" <<'PY'
import sys
from pathlib import Path

path, old, new = sys.argv[1], sys.argv[2], sys.argv[3]
text = Path(path).read_text()
in_section = False
out = []
replaced = False
for line in text.splitlines(keepends=True):
    stripped = line.lstrip()
    if stripped.startswith("["):
        in_section = stripped.startswith("[workspace.package]")
    if in_section and not replaced and stripped.startswith("version"):
        out.append(line.replace(f'"{old}"', f'"{new}"', 1))
        replaced = True
    else:
        out.append(line)
if not replaced:
    sys.stderr.write(f"could not find version line in {path}\n")
    sys.exit(1)
Path(path).write_text("".join(out))
PY
else
  printf '  [dry-run] sed-equivalent: [workspace.package] version "%s" → "%s"\n' "$current_version" "$new_version"
fi

# ─── Roll CHANGELOG.md ───────────────────────────────────────────────
# Replace `## [Unreleased]` header with `## [Unreleased]` (empty)
# followed by `## [NEW] - YYYY-MM-DD` containing the previous Unreleased
# block. Bottom-of-file reference links updated: [Unreleased] now
# points at `vNEW...HEAD`; a new `[NEW]` link points at `compare/vOLD...vNEW`.
if [[ "$SKIP_CHANGELOG" -eq 0 ]]; then
  log "Rolling $CHANGELOG"
  if [[ "$DRY_RUN" -eq 0 ]]; then
    python3 - "$CHANGELOG" "$current_version" "$new_version" <<'PY'
import sys
import re
from pathlib import Path
from datetime import date

path, old, new = sys.argv[1], sys.argv[2], sys.argv[3]
text = Path(path).read_text()
today = date.today().isoformat()

# Split body into the Unreleased block + everything after.
# We expect the structure:
#   ...
#   ## [Unreleased]
#   <body>
#   ## [vOlder] - DATE
#   ...
#   [Unreleased]: <repo>/compare/vOLD...HEAD
#   [vOLD]: <repo>/releases/tag/vOLD
match = re.search(
    r"^(?P<head>.*?)(?P<unreleased_marker>^## \[Unreleased\][^\n]*\n)"
    r"(?P<unreleased_body>.*?)(?=^## \[)",
    text,
    flags=re.DOTALL | re.MULTILINE,
)
if match is None:
    sys.stderr.write("CHANGELOG: could not locate ## [Unreleased] section\n")
    sys.exit(1)

head = match.group("head")
unreleased_body = match.group("unreleased_body")
rest = text[match.end():]

# Strip leading blank lines from the unreleased body so the new version
# header lands cleanly.
body_clean = unreleased_body.strip("\n")
if not body_clean.strip():
    sys.stderr.write("CHANGELOG: [Unreleased] block is empty — nothing to release\n")
    sys.exit(1)

new_unreleased = "## [Unreleased]\n\n"
new_version_block = f"## [{new}] - {today}\n\n{body_clean}\n\n"

new_body = head + new_unreleased + new_version_block + rest

# Bottom-of-file reference links — additive.
# 1. [Unreleased] line: vNEW...HEAD
# 2. Insert a new [NEW] line above the next entry (or append if absent).
def update_links(s: str) -> str:
    s = re.sub(
        r"^\[Unreleased\]:[^\n]*$",
        f"[Unreleased]: https://github.com/Nubaeon/ecodex/compare/v{new}...HEAD",
        s,
        count=1,
        flags=re.MULTILINE,
    )
    if f"[{new}]:" in s:
        return s  # already there
    new_link = f"[{new}]: https://github.com/Nubaeon/ecodex/compare/v{old}...v{new}"
    # Insert after the [Unreleased] line.
    return re.sub(
        r"^(\[Unreleased\]:[^\n]*\n)",
        rf"\g<1>{new_link}\n",
        s,
        count=1,
        flags=re.MULTILINE,
    )

new_body = update_links(new_body)
Path(path).write_text(new_body)
PY
  else
    printf '  [dry-run] would roll [Unreleased] → [%s] - %s\n' "$new_version" "$(date -I)"
  fi
fi

# ─── Gates (Phase 2 — opt-in) ────────────────────────────────────────
# Run AFTER bump + changelog roll (so the version under test is the
# version that's about to be tagged) and BEFORE commit + tag (so a
# failed gate leaves the working tree dirty for review — `git checkout
# -- codex-rs/Cargo.toml CHANGELOG.md` reverts cleanly).
#
# Each gate exits non-zero on failure. Cargo invocations stream output
# directly to the user's terminal so the failure is legible.
gate_build() {
  log "[gate-build] cargo build --release -p codex-cli -p codex-empirica-plugin -p codex-empirica-translator"
  if [[ "$DRY_RUN" -eq 1 ]]; then
    printf '  [dry-run] cargo build --release ...\n' >&2
    return 0
  fi
  (cd "${ECODEX_ROOT}/codex-rs" && \
    cargo build --release -p codex-cli -p codex-empirica-plugin -p codex-empirica-translator) \
    || error "gate-build failed — review cargo output, fix, then re-run"
}

gate_test() {
  log "[gate-test] cargo test --workspace --lib"
  if [[ "$DRY_RUN" -eq 1 ]]; then
    printf '  [dry-run] cargo test --workspace --lib\n' >&2
    return 0
  fi
  (cd "${ECODEX_ROOT}/codex-rs" && cargo test --workspace --lib) \
    || error "gate-test failed — review failures, fix, then re-run"
}

gate_clippy() {
  log "[gate-clippy] cargo clippy --workspace --all-targets"
  if [[ "$DRY_RUN" -eq 1 ]]; then
    printf '  [dry-run] cargo clippy --workspace --all-targets\n' >&2
    return 0
  fi
  (cd "${ECODEX_ROOT}/codex-rs" && cargo clippy --workspace --all-targets) \
    || error "gate-clippy failed — fix lints, then re-run"
}

if [[ "$GATE_BUILD" -eq 1 ]]; then gate_build; fi
if [[ "$GATE_TEST"  -eq 1 ]]; then gate_test;  fi
if [[ "$GATE_CLIPPY" -eq 1 ]]; then gate_clippy; fi

# ─── Commit ──────────────────────────────────────────────────────────
if [[ "$SKIP_COMMIT" -eq 0 ]]; then
  log "Staging + committing"
  run_or_dry git add "$CARGO_TOML" "$CHANGELOG"
  run_or_dry git commit -m "release: v${new_version}"
fi

# ─── Tag ─────────────────────────────────────────────────────────────
if [[ "$SKIP_TAG" -eq 0 && "$SKIP_COMMIT" -eq 0 ]]; then
  log "Tagging v${new_version}"
  run_or_dry git tag -a "v${new_version}" -m "ecodex v${new_version}"
fi

# ─── Push (Phase 2 — opt-in) ─────────────────────────────────────────
# Pushes the release commit on the current branch + the new tag. We
# do NOT push --follow-tags because that's all-or-nothing; an explicit
# two-step is more debuggable when the network or remote rejects.
if [[ "$PUSH" -eq 1 ]]; then
  if [[ "$SKIP_COMMIT" -eq 1 ]]; then
    warn "--push skipped because --skip-commit was passed (nothing to push)"
  else
    log "Pushing $current_branch + tag v${new_version} to origin"
    run_or_dry git push origin "$current_branch"
    if [[ "$SKIP_TAG" -eq 0 ]]; then
      run_or_dry git push origin "v${new_version}"
    fi
  fi
fi

# ─── GitHub release (Phase 2 — opt-in, requires gh CLI) ──────────────
# Creates the release shell with --generate-notes (gh derives notes
# from commits since the previous tag). For binary asset uploads, run
# `gh release upload v${new_version} <files>` separately after the
# Phase 2 build artifacts are produced. We don't auto-upload here
# because asset selection is too project-specific for a generic flag.
if [[ "$CREATE_GH_RELEASE" -eq 1 ]]; then
  if [[ "$SKIP_TAG" -eq 1 || "$SKIP_COMMIT" -eq 1 || "$PUSH" -eq 0 ]]; then
    warn "--create-gh-release requires committed + tagged + pushed state (skipping)"
  elif ! command -v gh >/dev/null 2>&1; then
    warn "gh CLI not found on PATH — install from https://cli.github.com/ to auto-create the release"
  else
    log "Creating GitHub release v${new_version}"
    run_or_dry gh release create "v${new_version}" \
      --title "ecodex v${new_version}" \
      --generate-notes
  fi
fi

# ─── Done ────────────────────────────────────────────────────────────
echo ""
if [[ "$DRY_RUN" -eq 1 ]]; then
  echo "✓ Dry run complete. Re-run without --dry-run to apply."
else
  echo "✓ Phase 1 complete."
  echo "  • Cargo.toml workspace.package.version → ${new_version}"
  echo "  • CHANGELOG.md rolled, [Unreleased] block promoted to [${new_version}]"
  if [[ "$SKIP_COMMIT" -eq 0 ]]; then
    echo "  • Commit: $(git rev-parse --short HEAD) (release: v${new_version})"
  fi
  if [[ "$SKIP_TAG" -eq 0 && "$SKIP_COMMIT" -eq 0 ]]; then
    echo "  • Tag:    v${new_version}"
  fi
fi
echo ""
echo "Phase 2 surfaces (opt-in via flags, all default off):"
echo "  --gate-build / --gate-test / --gate-clippy / --gate-all"
echo "  --push                   git push <branch> + tag"
echo "  --create-gh-release      gh release create with --generate-notes"
echo ""
echo "Phase 3 (still TODO):"
echo "  • cargo publish for own crates (codex-empirica-plugin, codex-empirica-translator)"
echo "  • homebrew Formula update (when we have a Tap)"
echo "  • npm package publish (if we ship a wrapper there)"
echo "  • binary asset uploads to the GH release (gh release upload v${new_version} <files>)"
