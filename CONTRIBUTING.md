# Contributing to ecodex

ecodex is the epistemic agent environment built on top of [openai/codex](https://github.com/openai/codex). This guide covers the contribution surfaces that are specific to ecodex; for changes that any codex user would want, contribute upstream.

## Where to contribute

We organize work in three layers. Pick the right one before opening a PR.

| Layer | Where | Examples | Where it lands |
|---|---|---|---|
| **L1 — codex foundation** | upstream `openai/codex` | agent runtime, sandbox, RPC, plugin host, hook system | upstream PR; ecodex picks it up on the next upstream sync |
| **L2 — empirica plugin** | `codex-rs/codex-empirica-plugin/` | hook routing, hook output translation, plugin provisioning, the vendored hooks and skills | PR against `main` here |
| **L3 — ecodex-specific** | everything else ecodex owns: the translator, fork touch-points in `codex-rs/`, `scripts/`, `ecodex/`, `docs/ecodex/` | wire-protocol translator, curated provider defaults, installers, branding, docs | PR against `main` here |

When in doubt: if your change benefits codex users with no opinion on Empirica, it's L1. If it's about Empirica discipline being expressed through codex, it's L2. Otherwise L3.

## Branches

**`main`** is where ecodex work happens and what PRs target. Upstream codex is tracked through the `upstream` remote and merged into `main` at a tagged upstream release; `codex-rs/UPSTREAM_SYNC_TAG` records which one, and CI fails if it falls behind the workspace version. Hardening that upstream would want goes back to them as a separate PR.

## Development workflow

### Build from source

```sh
git clone https://github.com/EmpiricaAI/ecodex.git
cd ecodex
./ecodex/scripts/install.sh
ecodex --version
```

The source installer builds the release binaries (`ecodex`, `codex-empirica-plugin`, `codex-empirica-translator`), installs them behind a small wrapper, and sets up the `empirica` CLI with pipx or uv when it is missing, like the prebuilt installer. The first time ecodex starts it writes the bundled plugin and a default `~/.codex/config.toml` itself. [`docs/ecodex/INSTALL.md`](docs/ecodex/INSTALL.md) covers the prebuilt channels, updating, and environment-specific notes.

### Iterate on the empirica plugin (L2)

The plugin lives at `codex-rs/codex-empirica-plugin/`. It is a library and a binary: the `ecodex` binary embeds the plugin's assets (hooks, skills, agents, `hooks.json`, the default config) and writes them to `~/.codex/plugins/cache/empiricaAI/empirica/` on startup; codex then runs the `codex-empirica-plugin` binary for each hook event, which runs the Python hook with the interpreter of the `empirica` CLI on PATH.

```sh
cd codex-rs
cargo build -p codex-empirica-plugin        # the hook host on PATH must be this build
just test -p codex-empirica-plugin
```

The vendored hooks under `assets/hooks_scripts/` come from Empirica and are not edited in place. Re-vendor them with `scripts/setup-codex.py` (dry-run by default; `--apply` to write, `--ref` to pick the empirica commit). It updates drifted files verbatim, lists new upstream files ecodex does not carry yet, flags model-facing Claude-isms, runs the vendored-hook tests, and stamps the vendored version and commit into `manifest.json`. After a re-vendor, set the empirica ref in `.github/workflows/ci.yml` to the stamped commit; CI fails until you do.

`empirica diagnose --frontend ecodex` is the source of truth for "is the empirica integration alive in this install."

### Iterate on the wire-protocol translator (L3)

The translator lives at `codex-rs/codex-empirica-translator/`. It serves codex's Responses API and forwards to providers that only speak Chat Completions or Anthropic Messages. CIF (Canonical Intermediate Format) is the internal abstraction; adapters convert protocol → CIF → protocol.

```sh
cd codex-rs
cargo run -p codex-empirica-translator      # routes from ~/.codex/translator-upstreams.toml, 127.0.0.1:18080
cargo run -p codex-empirica-translator -- --help
```

Adding a new adapter: implement the protocol → CIF and CIF → SSE-stream conversions, then wire it into `server.rs`. The adapter set (Chat Completions, Anthropic, native Responses passthrough) is what validates that CIF holds across protocol families. Model quirks that would make codex reject a tool call belong in `tool_args.rs`.

### Add a curated provider (L3)

The curated seed `codex-rs/models-manager/models.curated.json` is the one list: the `/model` picker is derived from it (`curated_seed::picker_entries`), and `ecodex models` reads it. `codex-rs/tui/src/ecodex_curated_models.rs` only turns seed entries into presets; nothing is hand-written there. A new picker entry needs:

1. A seed entry: `slug`, `display_name`, `description`, `context_window`, `supports_tools`, `reasoning`, `routes`, `jurisdiction`, `calibration_tier: "unmeasured"`, and `last_verified` + `evidence` saying how the context window was checked.
2. A `"picker": { "provider": "<id>", "category": "<cloud_coding | cloud_reasoning | local_open_weights | cloud_router>", "order": <n> }` block. `provider` is the `model_providers.<id>` the picker switches to with the model — it must exist, either built into codex (`openai`, `ollama`, `lmstudio`, …) or as a block in `config.toml.default`.
3. A `[model_providers.<id>]` block in `codex-rs/codex-empirica-plugin/assets/config/config.toml.default` when the provider is not built in (base URL, env key for the API key, wire API).

Entries without a `picker` block are still curated (context window, capabilities, `ecodex models` output) but not offered in the picker. Picker membership comes from the bundled seed only: `~/.codex/models.user.json` (written by `ecodex models refresh`) overlays the seed for capability lookups and wins on a slug collision, but it cannot add or remove picker rows.

codex speaks only the Responses API. A provider that serves it is reached directly. A provider that speaks only Chat Completions or Anthropic Messages is reached through the translator: its `base_url` points at `http://localhost:18080/v1`, and its models get a route in `codex-rs/codex-empirica-plugin/assets/config/translator-upstreams.toml`, the routes file ecodex writes for new installs. Nothing starts the translator automatically.

### Checks to run

```sh
cd codex-rs && just fmt                      # format
just test -p <crate>                         # the crate you changed
just fix -p <crate>                          # clippy fixes, scoped
cd .. && python3 scripts/check_vendored_firewall.py
python3 scripts/check_upstream_sync_tag.py
python3 scripts/check_empirica_core_pin.py
python -m pytest codex-rs/codex-empirica-plugin/tests/vendored_hooks/   # needs empirica importable
empirica compliance-report                   # lint, complexity, tests, docs, repo hygiene
```

`ruff.toml` at the repo root scopes Python lint to the code ecodex owns, so `compliance-report` does not report upstream's lint debt. [`AGENTS.md`](AGENTS.md) holds the full Rust conventions.

## Coding conventions

- **Don't break upstream surfaces.** ecodex does not rename, reorganize, or change the contract of an upstream type. We add layers; we don't divert. Every divergence costs us at every sync.
- **The plugin trust allowlist** in `codex-rs/hooks/src/engine/discovery.rs` (auto-trust for `empirica@empiricaAI`) is the one special case we accept inside upstream code. Other special cases need explicit discussion.
- **Vendored assets** under `codex-rs/codex-empirica-plugin/assets/hooks_scripts/` change only through `scripts/setup-codex.py`. Empirica is the source of truth; fix hook behavior there.
- **Cargo workspace:** new crates under `codex-rs/` register in `codex-rs/Cargo.toml`'s `[workspace] members`, and in `[workspace.dependencies]` when other crates use them. Dependency changes also refresh `MODULE.bazel.lock` (`just bazel-lock-update`).
- **Logging:** use `tracing::info!` / `warn!` with structured fields. Keep `eprintln!` for one-shot diagnostics (e.g. hook subprocess startup failures).
- **No new clippy regressions.** PRs that introduce warnings are asked to fix them or `#[allow]` with a justification comment.

## Testing requirements

- Rust changes: a test per new behavior; integration tests for cross-crate paths.
- Hook behavior: the hooks are Empirica's, so behavior changes and their tests go to [Empirica](https://github.com/EmpiricaAI/empirica) and arrive here with the next re-vendor. ecodex's own tests under `codex-rs/codex-empirica-plugin/tests/vendored_hooks/` pin what ecodex depends on: the payload shapes codex sends, the firewall invariants, the de-Claude genericization.
- Doctor checks for ecodex live in Empirica (`diagnose_ecodex.py`); run each new check against a real install and paste the output in the PR.

## Commits + PRs

- One coherent change per commit. Reverts should be clean.
- Title format: `<type>(<scope>): <summary>` where `<type>` is `feat`, `fix`, `docs`, `chore`, `refactor`, `test`, `ci`, etc., and `<scope>` is the affected area (e.g. `feat(plugin)`, `fix(install)`, `chore(lint)`).
- The body covers the *why*: what regression it prevents, what behavior changes, why this design over the alternatives, and how it was verified.

## Filing issues + PRs

- **Bug reports** — [`/issues/new`](https://github.com/EmpiricaAI/ecodex/issues/new/choose) → "Bug report". Asks for `empirica diagnose --frontend ecodex` output and the layer (L1/L2/L3) the bug lives in.
- **Feature requests** — same place → "Feature request". Layer and user story.
- **Upstream sync tracking** — same place → "Upstream sync". A checklist of ecodex divergences that need careful merge attention.
- **Pull requests** — `.github/pull_request_template.md` fills in a test-plan checklist that mirrors CI.

Blank issues are disabled. Discussion about the broader Empirica framework goes to [`EmpiricaAI/empirica` discussions](https://github.com/EmpiricaAI/empirica/discussions).

## CI

`.github/workflows/ci.yml` runs on every PR and every push to `main`:

- build and test of the owned crates (`codex-empirica-plugin`, `codex-empirica-translator`) and the Hugging Face integration contract
- the drift guards: vendored firewall invariants, `UPSTREAM_SYNC_TAG` against the workspace version, and the empirica ref against the vendored commit
- the vendored-hook tests, run against the exact empirica commit the hooks came from

Upstream codex's own test suites are out of scope for ecodex CI. Upstream's workflows are kept under `.github/workflows-upstream/` for reference when syncing.

## Releases

ecodex versions keep upstream's major.minor and count releases in the patch number, so a release on the current base is a patch bump. The flow:

1. `./scripts/release.sh --explicit <version> --push --publish-crates` — bumps the workspace version, refreshes `Cargo.lock`, rolls `CHANGELOG.md`'s `[Unreleased]` section, commits, tags, pushes, and publishes the owned crates (needs a crates.io token). `--dry-run` shows every step; `--gate-all` adds build, test and clippy gates before the commit.
2. The tag push runs `.github/workflows/release.yml`, which builds the four release targets (Linux and macOS, x86_64 and aarch64) and attaches them to the GitHub release. The macOS builds take the longest.
3. When the builds are attached, `scripts/sync-homebrew.sh <version> --tap <homebrew-tap checkout>` updates the Homebrew formula; commit and push it in the tap.
4. `./scripts/release.sh --explicit <version> --force-version --skip-commit --skip-tag --skip-changelog --verify-install` installs the published release into a scratch prefix and checks that it is a working, integrated ecodex: the right version, the plugin and its config written on first start, and the companion binaries.

## Security disclosures

Don't open public issues for security disclosures. See [`SECURITY.md`](SECURITY.md) for the disclosure path. ecodex inherits codex's threat model; ecodex-specific surfaces (the empirica plugin, the translator, the installers) are in scope for our disclosure process. The preferred channel is [GitHub Private Security Advisories](https://github.com/EmpiricaAI/ecodex/security/advisories/new).

## License

By contributing, you agree your changes ship under Apache-2.0 (the upstream license, which we inherit). See [`LICENSE`](LICENSE).
