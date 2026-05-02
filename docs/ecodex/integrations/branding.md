# ecodex Branding Swap

How to rebrand the codex binary as `ecodex` for our distribution.

## Summary

The Rust binary rename is **3 source changes + 1 Cargo entry**. The arg0 alias-dispatch trick is unaffected (it operates on distinct alias names, not the main binary name). The npm wrapper is optional and isolated.

## Surface area

### Required for Rust binary rename (4 changes)

| Location | Current | Change to |
|---|---|---|
| `codex-rs/cli/Cargo.toml:9` (`[[bin]] name`) | `name = "codex"` | `name = "ecodex"` |
| `codex-rs/cli/src/main.rs:82` (clap `bin_name`) | `bin_name = "codex"` | `bin_name = "ecodex"` |
| `codex-rs/cli/src/main.rs:83` (clap `override_usage`) | `"codex [OPTIONS] [PROMPT]\n       codex [OPTIONS] <COMMAND> [ARGS]"` | `"ecodex [OPTIONS] [PROMPT]\n       ecodex [OPTIONS] <COMMAND> [ARGS]"` |
| `codex-rs/cli/src/main.rs:1679` (`print_completion` shell-completion name) | `let name = "codex";` | `let name = "ecodex";` |

After these four edits, `cargo build --release -p codex-cli` produces `target/release/ecodex` instead of `target/release/codex`. Help output, usage strings, and shell completions all read `ecodex`.

### Cosmetic only (test inputs)

Many tests use `["codex", "exec", ...]` as clap parser argv. The literal `"codex"` here is just argv[0] in test fixtures — clap doesn't actually use it for anything. Search-and-replace to `"ecodex"` is purely cosmetic; behavior unchanged either way.

```
codex-rs/cli/src/main.rs:788, 1751, 1769, 1797, 1824, 1850,
                              1880, 1891, 1899, 1908, 1916, 1923, 1946,
                              1955, 1964, 1968, 1972, 1978, 1985, 2000,
                              2083, 2093, 2102, 2111
```

Decision: leave cosmetic test strings as-is. Avoids needless churn and keeps test diffs small.

### Optional: npm wrapper rename

The `codex-cli/` directory is a JS/TS wrapper distributed as `@openai/codex` on npm. If ecodex ships an npm wrapper too:

| Location | Current | Change to |
|---|---|---|
| `codex-cli/package.json:2` (`name`) | `"@openai/codex"` | `"@nubaeon/ecodex"` |
| `codex-cli/package.json:6` (bin entry) | `"codex": "bin/codex.js"` | `"ecodex": "bin/ecodex.js"` |
| `codex-cli/bin/codex.js` (file) | (file) | rename → `bin/ecodex.js` |
| `codex-cli/` (directory itself, optional) | (dir) | could rename → `ecodex-cli/` for consistency |

Defer until we decide whether ecodex has an npm distribution channel (vs. only GitHub Releases binary + cargo install). The Rust binary rename works standalone without touching the JS wrapper.

## What is *not* affected

### `codex-rs/arg0/` — alias dispatch

`arg0_dispatch()` checks argv[0] against compile-time-constant alias names (`codex-linux-sandbox`, `apply_patch`, `applypatch`, `codex-execve-wrapper`). These are *distinct* names from the main binary; they live as symlinks in a per-session temp dir under `~/.codex/tmp/arg0/`. Renaming the main binary from `codex` to `ecodex` does not change any of them.

The temp-dir path itself is hardcoded under `find_codex_home()` (i.e. `~/.codex/`). To put aliases under `~/.ecodex/` instead, a separate change in `utils/home-dir/` would be needed (see "CODEX_HOME inheritance" below).

### CODEX_HOME inheritance

`codex-rs/utils/home-dir/src/lib.rs` resolves `CODEX_HOME` env var, defaulting to `~/.codex/`. ecodex installations could:

- **Option A:** keep `~/.codex/` (config compatibility with codex; users with both share state). Simple, no code change.
- **Option B:** rename to `~/.ecodex/` (full isolation from codex). Requires forking `find_codex_home()` to read `ECODEX_HOME` first, fall back to `~/.ecodex/`. Breaks ability to share configs.
- **Option C:** new env var `ECODEX_HOME` overrides `CODEX_HOME`, default to `~/.ecodex/`. Best of both.

Recommendation: **Option A initially** (zero friction; users running both side-by-side can share config), with Option C as a future iteration if isolation becomes important.

### Subcommand naming

All codex subcommands (`exec`, `mcp`, `mcp-server`, `sandbox`, `plugin`, `marketplace`, etc.) keep their current names. Users will type `ecodex exec ...` instead of `codex exec ...` but the second-word vocabulary is unchanged.

### Plugin install path

`~/.codex/plugins/cache/<id>/<version>/` (per Option A above). If we go Option B/C, becomes `~/.ecodex/plugins/cache/...`.

## Verification plan

When the rebrand is applied, verify:

1. `cargo build --release -p codex-cli` produces `target/release/ecodex` (not `codex`).
2. `target/release/ecodex --help` shows `Usage: ecodex [OPTIONS] [PROMPT]`.
3. `target/release/ecodex --version` works.
4. `target/release/ecodex completion bash` outputs a `_ecodex` completion function.
5. Sandbox subcommands still work: `target/release/ecodex sandbox linux echo hi` should execute through landlock (alias dispatch unaffected).
6. Plugin discovery still finds `~/.codex/plugins/cache/empirica/` (or `~/.ecodex/...` if we picked Option B/C).
7. arg0 aliases (`apply_patch`, etc.) still resolve correctly when invoked as child processes.

## Implementation transaction (T?)

A future transaction will:
1. Apply the 4 required source edits
2. `cargo build --release` to confirm the rename produces `ecodex` binary
3. Run the 7 verification steps above
4. Decide on `~/.codex` vs `~/.ecodex` (likely Option A for v1)
5. Update plugin install docs to reflect any path changes
6. Commit on a `build/v1-distribution` or similar branch

Not done in this research transaction (T9) — research-only.
