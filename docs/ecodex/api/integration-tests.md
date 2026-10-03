# codex-empirica-plugin — Integration Tests

How the codex ↔ Empirica integration is tested, how to exercise a hook by hand,
and what the plugin's bring-up established about codex that still holds.

## What is tested, and where

The integration has four seams. Each has its own tests.

| Seam | What can break | Tests | Run |
|---|---|---|---|
| **codex → hook host** | the hook command, the payload, which interpreter runs the Python hook | `codex-rs/codex-empirica-plugin/src/` (unit tests in `empirica_cli.rs`, `translate_output.rs`, `provision_tests.rs`) | `cd codex-rs && just test -p codex-empirica-plugin` |
| **hook host → codex** | the hook's output shape: codex rejects unknown fields, fails open on some decisions, drops context it does not expect | `translate_output.rs` tests (PreToolUse `ask` → `deny`, nested `additionalContext` on SessionStart and PostToolUse) | same |
| **the vendored hooks themselves** | a re-vendor dropping a firewall invariant, a hook that does not handle codex's payload shape, a Claude-ism reaching the model | `codex-rs/codex-empirica-plugin/tests/vendored_hooks/` (pytest, against a real `empirica` install) and `scripts/check_vendored_firewall.py` | `python -m pytest codex-rs/codex-empirica-plugin/tests/vendored_hooks/` |
| **startup provisioning** | ecodex not writing the plugin, its config entry or the translator routes; the legacy key not migrating | `provision_tests.rs` (file layout, refresh, opt-out, migration) and `codex-rs/cli/tests/empirica_provision.rs` (the real `ecodex` binary in a scratch home) | `just test -p codex-empirica-plugin`, `just test -p codex-cli` |

CI (`.github/workflows/ci.yml`) runs the owned-crate tests, the drift guards,
and the vendored-hook tests against the exact empirica commit the hooks were
vendored from (`scripts/check_empirica_core_pin.py` keeps that pin honest).

A release is checked once more as users get it: `scripts/release.sh
--verify-install` installs the published release into a scratch prefix, starts
one session against a scratch `CODEX_HOME`, and requires the plugin, its config
entry, the translator routes and both companion binaries.

## Running a hook by hand

`codex-empirica-plugin` reads a codex hook payload on stdin, exactly as codex
sends it. Payload schemas are in `codex-rs/hooks/schema/generated/`.

```sh
# The typed handlers
echo '{"session_id":"00000000-0000-0000-0000-000000000001","hook_event_name":"PreToolUse",
       "tool_name":"Bash","tool_input":{"command":"ls"},"cwd":"/tmp/scratch-project"}' \
  | codex-empirica-plugin pre-tool-use

# Any vendored script, as hooks.json wires the sibling hooks
echo '{"session_id":"...","hook_event_name":"PostToolUse","tool_name":"Bash",
       "tool_input":{"command":"rg TODO | head -3"},"tool_response":"a\nb\nc\n"}' \
  | codex-empirica-plugin run-hook PostToolUse truncation-legibility.py
```

- `EMPIRICA_HOOKS_DIR` points the host at another hooks directory, such as
  `codex-rs/codex-empirica-plugin/assets/hooks_scripts/hooks` in a checkout, or a
  directory holding a probe script. A probe that records `sys.executable` and
  whether `import empirica` works is the quickest way to see which interpreter a
  given PATH gives the hooks.
- The host runs the Python hook with the interpreter named in the shebang of the
  `empirica` script on PATH (falling back to `python3`), so the PATH you test
  with matters. On a pipx or Homebrew empirica install, the first `python3` on
  PATH usually cannot import empirica.
- The host sets `EMPIRICA_INSTANCE_ID` to the payload's `session_id`, so a fresh
  `session_id` is a fresh empirica instance. The project is still resolved from
  `cwd`: for an isolated gating test, use a scratch directory with its own
  `.empirica/`, or the sentinel will find your real project's open transaction
  and decide by it.
- Codex treats a hook's output strictly. Run the host's output, not the Python
  script's, when checking what codex will see.

## What the bring-up established

These came out of getting the plugin to load and gate in the first place. They
are properties of codex, so they hold until upstream changes them.

- **The input side needs no translation.** codex's hook payload carries the
  fields Empirica's hooks read (`session_id`, `cwd`, `tool_name`, `tool_input`,
  `permission_mode`, `transcript_path`, `tool_use_id`). A finished shell call
  arrives as `tool_name: "Bash"`, with `tool_response` as a plain string. The
  output side does need translation, which is why every handler's output goes
  through `translate_output.rs`.
- **Plugin discovery is strict.** The manifest must be at
  `<plugin_root>/.codex-plugin/plugin.json` (or `.claude-plugin/plugin.json`);
  the install root is `<codex_home>/plugins/cache/<marketplace>/<plugin>/<version>/`;
  the key is `<plugin>@<marketplace>`, each segment alphanumeric, dash or
  underscore (`utils/plugins/src/plugin_namespace.rs`,
  `plugin/src/plugin_id.rs`). The empirica plugin is `empirica@empiricaAI`.
- **codex's only lock on plugin state is system-wide.** codex reads managed
  requirements from `/etc/codex/requirements.toml` on Unix and
  `%ProgramData%\OpenAI\Codex\requirements.toml` on Windows
  (`config/src/loader/mod.rs`). There is no per-user path, so nothing in a
  per-user install stops a runtime from setting the plugin's `enabled = false`;
  ecodex does not add a lock of its own.
- **codex speaks only the Responses API.** Upstream removed `wire_api = "chat"`,
  which is why ecodex has its own translator for Chat Completions and Anthropic
  providers.
- **Failure has to be loud in one direction and soft in the other.** A missing
  hook script fails open (the user opted out of the firewall); a hook script
  that is present but cannot be run fails closed for the firewall
  (`hook_script_exists` tells them apart). The sentinel itself allows on an
  internal exception and says so in its reason.
- **Check the loader before assuming a codex path or convention.** Every
  install-layout mistake in the bring-up (plugin key, cache layout, manifest
  location, lock location) was an assumption that reading the loader would have
  caught.
