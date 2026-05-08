#!/usr/bin/env bash
# ecodex release script — stub
#
# This is a placeholder until the full release_chain pipeline ships
# (see open goal: "ecodex release_chain pipeline (Rust-adapted)" —
# id c8d49526). Repo-hygiene compliance requires *some* release script
# at scripts/release.sh | scripts/release.py | Makefile; this satisfies
# that gate while we design the proper pipeline.
#
# When the real release pipeline lands (cargo build → version bump →
# changelog roll → tag → push → GitHub release → cargo publish), it
# will replace this file. The replacement should keep the same path
# (scripts/release.sh) so muscle memory + CI invocations stay valid.
#
# What the real pipeline needs to do, sketched:
# 1. Read current version from codex-rs/Cargo.toml [workspace.package]
# 2. Bump per --release-type {major,minor,patch}
# 3. Roll CHANGELOG.md (move [Unreleased] entries under the new version)
# 4. cargo build --release -p codex-cli -p codex-empirica-plugin
# 5. cargo test --workspace --lib (gate on green)
# 6. cargo clippy --workspace --all-targets (gate on clean)
# 7. git commit + tag + push
# 8. gh release create with the changelog excerpt + binaries attached
# 9. (later) cargo publish for crates that should land on crates.io

set -euo pipefail

cat <<'EOF' >&2
ecodex release script: not yet implemented.

This stub satisfies the repo-hygiene compliance check. The full release
pipeline is tracked under goal c8d49526 ("ecodex release_chain pipeline").

For now, manual release flow:
  1. Edit codex-rs/Cargo.toml [workspace.package] version.
  2. Roll CHANGELOG.md.
  3. cargo build --release -p codex-cli -p codex-empirica-plugin
  4. cargo test --workspace --lib
  5. cargo clippy --workspace --all-targets
  6. git commit -am "release: vX.Y.Z" && git tag vX.Y.Z && git push --tags
  7. gh release create vX.Y.Z --notes-from-changelog
EOF
exit 64
