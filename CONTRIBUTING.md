# Contributing to ecodex

ecodex is the epistemic agent environment built on top of [openai/codex](https://github.com/openai/codex). This guide covers the contribution surfaces that are specific to ecodex; for codex-internal changes, contribute upstream.

## Where to contribute

We organize work in three layers — pick the right one before opening a PR.

| Layer | Repo | Examples | Where it lands |
|---|---|---|---|
| **L1 — codex foundation** | upstream `openai/codex` | agent runtime, sandbox, RPC, plugin host, hook system | upstream PR; we sync via `main` rebase |
| **L2 — empirica plugin** | this repo, `codex-rs/codex-empirica-plugin/` | hook routing, transaction lifecycle, AGENTS.md seeding, sentinel firewall | direct PR against `build/v1-plugin` |
| **L3 — ecodex-specific** | this repo (everything else under `codex-rs/`, `ecodex/`, `docs/ecodex/`) | wire-protocol translator, curated provider defaults, install scripts, branding, docs | direct PR against `build/v1-plugin` |

When in doubt: if your change benefits codex users with no opinion on Empirica, it's L1 (upstream). If it's about Empirica discipline being expressed through codex, it's L2. Otherwise L3.

## Branches

- **`main`** — upstream tracking only. No ecodex-specific commits. Use this branch when forwarding fixes upstream.
- **`build/v1-plugin`** (default) — active ecodex work. PRs target this.

We rebase `main` onto `upstream/main` and merge selected hardening commits upstream.

## Development workflow

### Build + smoke-test

```sh
git clone https://github.com/Nubaeon/ecodex.git
cd ecodex
./ecodex/scripts/install.sh
ecodex --version
```

The install script auto-builds on first run, drops a wrapper at `~/.local/bin/ecodex`, installs the empirica plugin to `~/.codex/plugins/cache/`, and seeds `~/.codex/config.toml` with curated provider defaults if no config exists. See [`docs/ecodex/INSTALL.md`](docs/ecodex/INSTALL.md) for environment-specific notes.

### Iterate on the empirica plugin (L2)

The plugin lives at `codex-rs/codex-empirica-plugin/`. It's a thin Rust binary that codex invokes for each hook event (`PreToolUse`, `SessionStart`, etc.) and shells out to the empirica Python framework via subprocess.

```sh
cargo build --release -p codex-empirica-plugin
./ecodex/scripts/install.sh   # re-syncs the binary + bundled assets
```

The bundled assets (hook scripts, system prompt, subagents) are vendored from the empirica master into `codex-rs/codex-empirica-plugin/assets/` via `scripts/sync-empirica-assets.sh`. The doctor (`empirica diagnose-ecodex`) WARNs on drift.

### Iterate on the wire-protocol translator (L3)

The translator lives at `codex-rs/codex-empirica-translator/`. It bridges codex's Responses API to providers that only speak Chat Completions or Anthropic Messages. CIF (Canonical Intermediate Format) is the internal abstraction; adapters convert protocol → CIF → protocol.

```sh
cargo run --release -p codex-empirica-translator -- --upstream-protocol chat
```

Adding a new adapter: implement the protocol → CIF and CIF → SSE-stream conversions, then wire it into `server.rs`. The N=3 adapter set (chat-completions, Anthropic, native Responses passthrough) validates that CIF holds across protocol families.

### Add a curated provider entry (L3)

Curated entries live in `codex-rs/tui/src/ecodex_curated_models.rs` and `ecodex/config.toml.default`. Each entry needs:

1. A `ModelPreset` definition (display name, description, slug, default reasoning effort).
2. A `[model_providers.<name>]` block in `config.toml.default` (base URL, env key for API key, wire API).
3. A `provider_for_slug` mapping if the model routes to a different provider than the slug suggests.

When the wire API is `responses`, codex talks directly to the provider. When it's `chat` or `anthropic`, the wrapper auto-spawns the translator and rewrites the base URL to `localhost:18080`.

### Run the compliance + diagnostic suites

```sh
empirica compliance-report   # Lint, complexity, tests, tech_docs, repo_hygiene
empirica diagnose-ecodex     # Plugin install state, hook firing, statusline, translator health
cargo test --workspace --lib # Rust test suite
cargo clippy --workspace --all-targets
```

`compliance-report` excludes upstream codex paths via `ecodex/ruff.toml`. The doctor (`diagnose-ecodex`) is the source of truth for "is the empirica integration alive in the current install."

## Coding conventions

- **Don't break upstream surfaces.** ecodex never renames, reorganizes, or changes the contract of an upstream type. We add layers; we don't divert. Forking divergence costs us at every sync.
- **Tx-AT allowlist for plugin trust** is the only special-case patch we accept inside upstream code (`codex-rs/hooks/src/engine/discovery.rs`). Other special cases need explicit discussion.
- **Vendored assets** (`codex-rs/codex-empirica-plugin/assets/`) must be updated via `scripts/sync-empirica-assets.sh`, not edited in place. The empirica master is the source of truth.
- **Cargo workspace:** new crates added under `codex-rs/` must register in `codex-rs/Cargo.toml`'s `[workspace] members` and the `[workspace.dependencies]` table when shared by other crates.
- **Logging:** use `tracing::info!` / `warn!` with structured fields. Keep `eprintln!` for one-shot diagnostics (e.g. plugin subprocess startup failures).
- **No new clippy regressions.** Workspace `cargo clippy --workspace --all-targets` exits cleanly. PRs that introduce errors will be asked to fix or `#[allow]` with a justification comment.

## Testing requirements

- Rust changes: at least one unit test per new behavior; integration tests for cross-crate paths.
- Plugin hook changes: reach for the existing pytest scaffold under `empirica/tests/plugins/claude-code-integration/` (master repo, not vendored copies) and add cases there. Vendored assets in ecodex pick up the change via the next sync.
- Doctor checks: each new `check_ecodex_*` function in `empirica/cli/command_handlers/diagnose_ecodex.py` ships with a manual smoke run against the current install — paste the output in the PR description.

## Commits + PRs

- One transaction = one commit. Commits should be atomic; reverts should be clean.
- Title format: `<type>(<scope>): <summary>` where `<type>` is `feat`, `fix`, `docs`, `chore`, `refactor`, `test`, etc., and `<scope>` is the affected area (e.g. `feat(plugin)`, `fix(install)`, `chore(lint)`).
- Body covers the *why* — what regression were we preventing, what behavior were we changing, why this design over alternatives.
- Sign commits with the empirica session ID where applicable (commit messages auto-include this when authored via the empirica transaction lifecycle).

## Filing issues + PRs

We use templates to keep issue triage and PR review fast:

- **Bug reports** — [`/issues/new`](https://github.com/Nubaeon/ecodex/issues/new/choose) → "Bug report". Asks for `empirica diagnose-ecodex` output + the layer (L1/L2/L3) the bug lives in.
- **Feature requests** — same place → "Feature request". Layer + user story.
- **Upstream sync tracking** — same place → "Upstream sync". Includes a checklist of ecodex divergences (T78 hot-swap, Tx-AT trust allowlist, hook output translation, etc.) that need careful merge attention.
- **Pull requests** — `.github/pull_request_template.md` auto-fills. Includes the test-plan checklist that mirrors CI (`cargo build` / `cargo test` / `cargo clippy` on owned crates).

Blank issues are disabled — pick a template. Discussions about the broader Empirica framework go to [`Nubaeon/empirica` discussions](https://github.com/Nubaeon/empirica/discussions).

## CI

The `.github/workflows/ci.yml` workflow runs on every push to `main` / `build/v1-plugin` and every PR targeting either. It mirrors `scripts/release.sh`'s gate logic:

- `cargo build --release` for the three owned crates
- `cargo test --lib` scoped to owned crates (upstream codex tests are out of scope — see goal `0309b0ad`)
- `cargo clippy --workspace --all-targets`

Cache is keyed on `Cargo.lock` hash. Stack size is bumped to 16MB (`RUST_MIN_STACK=16777216`) to avoid the recursive-test SIGABRT we hit on v0.0.1's first cut.

The full upstream codex CI suite (bazel, rust-ci-full, V8 release, etc.) is archived under `.github/workflows-upstream/` for reference. When pulling upstream changes, the upstream-sync issue template walks through what to consider.

## Security disclosures

Don't open public issues for security disclosures. See [`SECURITY.md`](SECURITY.md) for the disclosure path. ecodex inherits codex's threat model; ecodex-specific surfaces (the empirica plugin, the translator, install scripts) are in scope for our disclosure process. The preferred channel is [GitHub Private Security Advisories](https://github.com/Nubaeon/ecodex/security/advisories/new).

## License

By contributing, you agree your changes ship under Apache-2.0 (the upstream license, which we inherit). See [`LICENSE`](LICENSE).
