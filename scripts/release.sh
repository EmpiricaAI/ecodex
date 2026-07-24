#!/usr/bin/env bash
# ecodex release pipeline
#
# Phase 1 (Tx-BC): version bump → CHANGELOG roll → commit → tag.
# Phase 2 (Tx-BD): opt-in build/test/clippy gates + push + gh release.
# Phase 3 (Tx-BE, this script): cargo publish own crates + binary asset uploads;
#   homebrew + npm informative stubs (require Tap repo + npm package setup we
#   don't yet have).
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
#   ./scripts/release.sh --patch --gate-all --push --create-gh-release \
#                        --upload-assets --publish-crates
#                                               Full Phase 1+2+3 flow.
#                                               Requires: gh CLI auth'd,
#                                               cargo build artifacts in
#                                               target/release/, and
#                                               CARGO_REGISTRY_TOKEN env
#                                               for crates.io publish.
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
UPLOAD_ASSETS=0
PUBLISH_CRATES=0
PUBLISH_HOMEBREW=0
PUBLISH_NPM=0
FORCE_VERSION=0

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
    --upload-assets)    UPLOAD_ASSETS=1;        shift ;;
    --publish-crates)   PUBLISH_CRATES=1;       shift ;;
    --publish-homebrew) PUBLISH_HOMEBREW=1;     shift ;;
    --publish-npm)      PUBLISH_NPM=1;          shift ;;
    --force-version)    FORCE_VERSION=1;        shift ;;
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
  if [[ "$FORCE_VERSION" -eq 1 ]]; then
    # Recovery path: pipeline failed mid-flight, version already bumped,
    # tag may already exist + be pushed. User wants to re-run just the
    # publish phases (gh release, cargo publish, brew tap) without
    # re-bumping. Combine with --skip-tag --skip-commit --skip-changelog
    # to skip everything in Phase 1.
    warn "computed version $new_version equals current — --force-version is set, proceeding (recovery mode)"
  else
    error "computed version $new_version equals current — nothing to release (use --force-version + --skip-{tag,commit,changelog} for recovery)"
  fi
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
        f"[Unreleased]: https://github.com/EmpiricaAI/ecodex/compare/v{new}...HEAD",
        s,
        count=1,
        flags=re.MULTILINE,
    )
    if f"[{new}]:" in s:
        return s  # already there
    new_link = f"[{new}]: https://github.com/EmpiricaAI/ecodex/compare/v{old}...v{new}"
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
  # Scope: our owned crates + the binary crate (codex-cli) that produces the
  # `ecodex` binary. We do NOT run --workspace --lib: upstream codex's 100+
  # crates contain tests that fail in this environment for reasons unrelated
  # to ecodex (env-dependent config snapshots, network-proxy timeouts).
  # Same scoping principle PUBLISH_ORDER uses. If a regression in upstream
  # code breaks ecodex, gate-build catches it (build fails). RUST_MIN_STACK
  # belt-and-suspenders for any future recursive test we add ourselves.
  local pkgs="-p codex-empirica-plugin -p codex-empirica-translator -p codex-cli"
  log "[gate-test] cargo test ${pkgs} --lib  (RUST_MIN_STACK=16M, owned crates only)"
  if [[ "$DRY_RUN" -eq 1 ]]; then
    printf '  [dry-run] RUST_MIN_STACK=16777216 cargo test %s --lib\n' "$pkgs" >&2
    return 0
  fi
  # shellcheck disable=SC2086  # word-splitting on $pkgs is intentional
  (cd "${ECODEX_ROOT}/codex-rs" && RUST_MIN_STACK=16777216 cargo test $pkgs --lib) \
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
    # --repo EmpiricaAI/ecodex: gh defaults to the parent repo for forks
    # (openai/codex), which doesn't have our tag and rejects with
    # "tag exists locally but has not been pushed to openai/codex".
    log "Creating GitHub release v${new_version}"
    run_or_dry gh release create "v${new_version}" \
      --repo EmpiricaAI/ecodex \
      --title "ecodex v${new_version}" \
      --generate-notes
  fi
fi

# ─── Phase 3: binary asset uploads (opt-in, requires gh release exists) ──
# Uploads the standard ecodex binary set from codex-rs/target/release/
# to the GH release shell created above. Asset paths must exist —
# users are responsible for running cargo build --release first (or
# combining with --gate-build, which produces these binaries as a side
# effect). Skipped with warning when prerequisites aren't met.
ECODEX_RELEASE_BIN="${ECODEX_ROOT}/codex-rs/target/release/ecodex"
PLUGIN_RELEASE_BIN="${ECODEX_ROOT}/codex-rs/target/release/codex-empirica-plugin"
TRANSLATOR_RELEASE_BIN="${ECODEX_ROOT}/codex-rs/target/release/codex-empirica-translator"

if [[ "$UPLOAD_ASSETS" -eq 1 ]]; then
  if [[ "$CREATE_GH_RELEASE" -eq 0 && "$DRY_RUN" -eq 0 ]]; then
    warn "--upload-assets needs --create-gh-release (or an existing release) — skipping"
  elif ! command -v gh >/dev/null 2>&1; then
    warn "gh CLI not found — skipping asset upload"
  else
    # Asset names embed the platform slug so the npm postinstall
    # (npm/scripts/postinstall.js) can fetch the right binary per-OS.
    # Slug shape: <os>-<arch> matching the postinstall's mapping.
    case "$(uname -s)" in
      Linux)  os_slug="linux" ;;
      Darwin) os_slug="macos" ;;
      *)      error "unsupported OS for asset upload: $(uname -s)" ;;
    esac
    case "$(uname -m)" in
      x86_64|amd64)  arch_slug="x86_64" ;;
      arm64|aarch64) arch_slug="aarch64" ;;
      *)             error "unsupported arch for asset upload: $(uname -m)" ;;
    esac
    platform_slug="${os_slug}-${arch_slug}"
    log "Uploading binary assets to v${new_version} (platform: ${platform_slug})"

    # Build asset list. Each binary gets renamed via gh's local#alias
    # form (gh release upload <tag> path/to/file#alias-name).
    asset_args=()
    for src_path in \
      "$ECODEX_RELEASE_BIN:ecodex" \
      "$PLUGIN_RELEASE_BIN:codex-empirica-plugin" \
      "$TRANSLATOR_RELEASE_BIN:codex-empirica-translator"
    do
      file="${src_path%%:*}"
      base="${src_path##*:}"
      if [[ -f "$file" ]]; then
        # gh release upload syntax: file#displayname
        asset_args+=("${file}#${base}-${platform_slug}")
      else
        warn "missing build artifact: $file (run cargo build --release or --gate-build first)"
      fi
    done
    if [[ "${#asset_args[@]}" -gt 0 ]]; then
      run_or_dry gh release upload "v${new_version}" "${asset_args[@]}" --clobber --repo EmpiricaAI/ecodex
    else
      warn "no assets to upload — skipping"
    fi
  fi
fi

# ─── Phase 3: cargo publish (opt-in, requires CARGO_REGISTRY_TOKEN) ──
# Publishes ecodex's own crates to crates.io. We deliberately list the
# crates we author — never auto-iterate workspace members because most
# of the workspace is upstream codex's surface (their owners' choice
# what to publish + when). Order matters: dependencies-first.
PUBLISH_ORDER=("codex-empirica-translator" "codex-empirica-plugin")

if [[ "$PUBLISH_CRATES" -eq 1 ]]; then
  if [[ -z "${CARGO_REGISTRY_TOKEN:-}" && "$DRY_RUN" -eq 0 ]]; then
    warn "CARGO_REGISTRY_TOKEN not set — get one at https://crates.io/me + export, or skip --publish-crates"
  else
    for crate in "${PUBLISH_ORDER[@]}"; do
      log "[publish-crates] cargo publish -p ${crate}"
      if [[ "$DRY_RUN" -eq 1 ]]; then
        printf '  [dry-run] cargo publish -p %s\n' "$crate" >&2
      else
        (cd "${ECODEX_ROOT}/codex-rs" && cargo publish -p "$crate") \
          || error "cargo publish $crate failed — fix metadata gaps then re-run with --publish-crates only (other phases are idempotent or already done)"
      fi
    done
  fi
fi

# ─── Phase 3: homebrew Formula update ────────────────────────────────
# Updates Formula/ecodex.rb in EmpiricaAI/homebrew-tap to point at the new
# release tarball. Requires the GH release to exist (sha256 is computed
# against the tarball gh auto-generates from the tag). Clones the Tap
# into a temp dir, edits the formula, commits + pushes.
HOMEBREW_TAP="EmpiricaAI/homebrew-tap"
HOMEBREW_FORMULA="ecodex.rb"

if [[ "$PUBLISH_HOMEBREW" -eq 1 ]]; then
  if [[ "$CREATE_GH_RELEASE" -eq 0 && "$DRY_RUN" -eq 0 ]]; then
    warn "--publish-homebrew needs --create-gh-release (sha256 derived from the release tarball) — skipping"
  elif ! command -v gh >/dev/null 2>&1; then
    warn "gh CLI not found — skipping homebrew update"
  else
    log "Updating $HOMEBREW_TAP/Formula/$HOMEBREW_FORMULA → v${new_version}"
    if [[ "$DRY_RUN" -eq 1 ]]; then
      printf '  [dry-run] clone %s, write Formula/%s pointing at v%s, commit + push\n' \
        "$HOMEBREW_TAP" "$HOMEBREW_FORMULA" "$new_version" >&2
    else
      tap_dir="$(mktemp -d)"
      trap 'rm -rf "$tap_dir"' EXIT INT TERM
      git clone --depth 1 "https://github.com/${HOMEBREW_TAP}.git" "$tap_dir" \
        || error "couldn't clone $HOMEBREW_TAP — does it exist?"
      # Generate the formula from the SINGLE authoritative source
      # (packaging/homebrew/ecodex.rb — a PREBUILT-BINARY formula) via
      # sync-homebrew.sh, which fills the per-platform SHA-256s from the
      # release's ecodex-<target>.tar.gz.sha256 assets and installs all three
      # binaries (ecodex + codex-empirica-plugin + codex-empirica-translator).
      # Requires the prebuilt binaries to be uploaded first (release.yml /
      # --upload-assets); the sync script fails loudly if an asset is missing.
      # NOTE: previously this wrote a divergent source-build formula inline,
      # which clobbered the prebuilt tap formula and dropped the plugin +
      # translator binaries — see scripts/sync-homebrew.sh for the canonical form.
      "${SCRIPT_DIR}/sync-homebrew.sh" "${new_version}" --tap "$tap_dir" \
        || error "sync-homebrew failed — are the v${new_version} release binaries uploaded?"

      (cd "$tap_dir" && \
        git add "Formula/${HOMEBREW_FORMULA}" && \
        git commit -m "ecodex v${new_version}" && \
        git push) || error "homebrew tap push failed — check EmpiricaAI/homebrew-tap permissions"
      rm -rf "$tap_dir"
      trap - EXIT INT TERM
    fi
  fi
fi

# ─── Phase 3: npm package publish ────────────────────────────────────
# Syncs npm/package.json version to the release version and runs
# `npm publish --access public`. The wrapper (npm/bin/ecodex.js) execs
# into a binary that npm/scripts/postinstall.js downloads from the GH
# release on install. So the GH release MUST exist with binary assets
# (--upload-assets) before this step runs in anger.
NPM_DIR="${ECODEX_ROOT}/npm"

if [[ "$PUBLISH_NPM" -eq 1 ]]; then
  if [[ ! -d "$NPM_DIR" ]]; then
    warn "npm/ directory missing — skipping npm publish"
  elif ! command -v npm >/dev/null 2>&1; then
    warn "npm not found on PATH — install Node.js to publish, or skip --publish-npm"
  elif [[ "$UPLOAD_ASSETS" -eq 0 && "$DRY_RUN" -eq 0 ]]; then
    warn "--publish-npm needs --upload-assets (postinstall downloads binaries from the release) — skipping"
  else
    log "Syncing $NPM_DIR/package.json version to ${new_version}"
    if [[ "$DRY_RUN" -eq 1 ]]; then
      printf '  [dry-run] sed-update package.json "version" → "%s"\n' "$new_version" >&2
      printf '  [dry-run] (cd %s && npm publish --access public)\n' "$NPM_DIR" >&2
    else
      python3 - "$NPM_DIR/package.json" "$new_version" <<'PY'
import json
import sys
from pathlib import Path

path, new_version = sys.argv[1], sys.argv[2]
data = json.loads(Path(path).read_text())
data["version"] = new_version
Path(path).write_text(json.dumps(data, indent=2) + "\n")
PY
      log "Publishing @nubaeon/ecodex@${new_version}"
      (cd "$NPM_DIR" && npm publish --access public) \
        || error "npm publish failed — check NPM_TOKEN env, npm whoami, package.json"

      # Re-stage + commit the version bump in npm/package.json so the
      # repo state matches the published version. Folded into the
      # release commit on the next run.
      (cd "$ECODEX_ROOT" && git add "$NPM_DIR/package.json")
      log "npm/package.json version sync staged — fold into next release commit if needed"
    fi
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
echo "Phase 3 surfaces (canonical):"
echo "  --upload-assets          gh release upload <ecodex,plugin,translator> binaries"
echo "  --publish-crates         cargo publish own crates (translator + plugin)"
echo "  --publish-homebrew       Update Formula in EmpiricaAI/homebrew-tap"
echo ""
echo "Phase 3 surfaces (experimental, non-canonical):"
echo "  --publish-npm            npm publish @nubaeon/ecodex — kept for future, not part of canonical release flow"
echo ""
echo "Recovery:"
echo "  --force-version          bypass the 'version unchanged' guard"
echo "                             (combine with --skip-{tag,commit,changelog})"
echo "                             use when a previous cut failed mid-flight"
echo "                             and you want to re-run just publish phases"
echo ""
echo "Canonical v0.x invocation:"
echo "  ./scripts/release.sh --patch --gate-all --push --create-gh-release \\"
echo "    --upload-assets --publish-crates --publish-homebrew"
