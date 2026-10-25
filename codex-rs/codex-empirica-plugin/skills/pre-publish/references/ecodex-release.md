# Releasing ecodex

ecodex keeps upstream codex's major.minor and counts its own releases in the patch.
The minor must equal the one in `codex-rs/UPSTREAM_SYNC_TAG`
(`scripts/check_upstream_sync_tag.py` fails CI otherwise), so a release between
upstream syncs is a patch bump. Derive the version from that file before you name
it to anyone. Commands run from the repo root on `main`. Cut only on the owner's
explicit go for this version.

**Name the repository on every `gh` call: `-R EmpiricaAI/ecodex`.** This checkout
has an `upstream` remote, and with no default set `gh` resolves to openai/codex. A
`gh run list` aimed there returns an empty list that reads exactly like "CI has not
started yet". `scripts/release.sh` already passes `--repo` for the same reason.

## Gates (the skill's steps 2 to 7)

- **`./scripts/broccoli.sh`** is the deterministic tier for what ecodex owns:
  rustfmt, clippy with `-D warnings`, the owned-crate tests, the vendored-firewall,
  UPSTREAM_SYNC_TAG and Hugging Face guards, and the vendored-hook suite. Its
  dependency advisories (cargo audit, deny, machete) are informational because they
  cover upstream's shared graph. Read them anyway: an advisory with a published fix
  in a crate the lockfile can move is a lockfile bump, not a note.
- **Run tests through `just test -p <crate> -E '<nextest filter>'`.** It uses the
  toolchain pinned in `codex-rs/rust-toolchain.toml`. A toolchain override once
  produced a snapshot failure that was not a defect.
- **`python3 scripts/setup-codex.py --ref v<empiricaVendorVersion>`**, with the
  version read from `codex-rs/codex-empirica-plugin/manifest.json`, should report
  no drifted files. It is a dry run unless given `--apply`, and it also flags
  host-specific wording in the skills. Without `--ref` it compares against
  empirica's `develop`, which shows what the next fold-in would bring, not drift.
- **A workflow edit is linted before it is pushed:**
  `actionlint .github/workflows/ci.yml .github/workflows/release.yml`. CI runs the
  same scoped step; upstream's own workflows are out of scope because they reference
  files this fork pruned. An unlinted job-level `if:` that read `matrix` once made
  the whole release workflow invalid.
- **Sweep areas** for an ecodex release are usually: the owned crates
  (`codex-empirica-plugin`, `codex-empirica-translator`, ecodex's patches in cli,
  tui and core), the vendored hooks and the bundled skills, workflows and scripts,
  and docs.
- **Hot path (step 5).** Each hook runs as `codex-empirica-plugin <event>` with
  codex's hook JSON on stdin. Time the last tag's installed binary against the new
  build with the same payload, 15 runs each. `session-start` and `pre-tool-use` are
  the two that run often enough to matter.

## Cut

1. **Stamp the README** "latest release" link with the new version, commit, push,
   and wait for CI on that commit: `gh run list -R EmpiricaAI/ecodex --commit
   <full sha>`, then `gh run watch -R EmpiricaAI/ecodex <run id>`.
2. **`./scripts/release.sh --explicit <version> --push --publish-crates`** bumps the
   workspace version, refreshes `Cargo.lock`, rolls the changelog's `[Unreleased]`
   section, commits, tags, pushes, and publishes the owned crates. Add `--dry-run`
   first if anything about the tree is unusual. cargo reads the crates.io token from
   its own credential store as well as `CARGO_REGISTRY_TOKEN`, so an unset variable
   does not mean there is no token. Never print the token or write it into a file.
3. **The tag push runs `release.yml`**: a plan job, then four builds (Linux and
   macOS, x86_64 and aarch64). Find the run with `gh run list -R EmpiricaAI/ecodex
   --workflow release.yml` and wait on its id; the macOS builds take longest. Rebuild one
   failed target with `gh workflow run release.yml -R EmpiricaAI/ecodex -f
   tag=v<version> -f targets=<target>` rather than repeating all four. The x86_64 Linux build runs
   near the runner's memory ceiling, which is why its `cargo_jobs` is low; an
   out-of-memory kill there reads as a dead rustc, not a compile error.
4. **Homebrew.** When all four `ecodex-<target>.tar.gz` archives and their
   `.sha256` files are on the release, run `scripts/sync-homebrew.sh <version> --tap
   <homebrew-tap checkout>`. Compare each sha256 in the filled formula with the
   published `.sha256` file before committing, because the formula is what users
   install. Commit the tap as `ecodex <version>` and push it.
5. **Verify the install:** `./scripts/release.sh --explicit <version>
   --force-version --skip-commit --skip-tag --skip-changelog --verify-install`
   installs the published release into a scratch prefix and checks that it is an
   integrated ecodex: the version, the plugin and config written on first start,
   and the companion binaries.
6. **Commit the filled formula** `packaging/homebrew/ecodex.filled.rb` on `main`.

## What shipped, read from where it shipped

- The GitHub release carries four archives and four `.sha256` files:
  `gh release view v<version> -R EmpiricaAI/ecodex --json assets`.
- crates.io carries `codex-empirica-plugin` and `codex-empirica-translator` at the
  version.
- The tap's `Formula/ecodex.rb` carries the version and the same four sha256.
- A machine running ecodex picks up the new bundled plugin on its next session
  start, when the plugin cache's bundle marker changes. Sessions already running
  keep what they loaded.
