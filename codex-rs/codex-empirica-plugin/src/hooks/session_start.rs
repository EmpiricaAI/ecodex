//! SessionStart hook → Empirica session bootstrap.
//!
//! Fires once when codex starts a new session. Forwards codex's SessionStart
//! JSON payload to `session-init.py` (which initializes Empirica state for
//! the session: writes the active-session pointer, loads project context,
//! installs auxiliary hooks, etc.).
//!
//! Same fail-open semantics as the other hooks.

use std::io::Read;
use std::process::ExitCode;

use crate::agents_md;
use crate::empirica_cli;
use crate::practice_bootstrap;
use crate::subagents;

/// Run the SessionStart hook against the current invocation.
///
/// Fires once when codex starts a new session (or resumes one). Three
/// side effects, in order:
/// 1. **AGENTS.md seed** — ensures `~/.codex/AGENTS.md` carries the
///    empirica system-prompt block so the model has discipline priming
///    when codex loads `user_instructions`. Idempotent; only writes
///    when content drift is detected.
/// 2. **Subagent seed** — copies bundled empirica subagents
///    (architecture, security, ux, etc.) into
///    `<codex_home>/agents/empirica/` so codex's Agent tool can
///    dispatch to them.
/// 3. **`session-init.py`** — bootstraps empirica's session-level
///    state (writes the active-work pointer, loads project context,
///    detects existing/orphaned sessions, returns the
///    `additionalContext` block injected into the agent's first turn).
///
/// All three are fail-open: a write/seed failure logs to stderr and
/// the session continues.
pub fn handle() -> ExitCode {
    // Ensure ~/.codex/AGENTS.md carries the empirica system-prompt block so
    // the codex agent has cognitive priming (identity + 13 vectors +
    // transaction discipline) when the model loads user_instructions.
    // Fail-open: if the seed fails, log and proceed — never block session boot.
    match agents_md::ensure_agents_md_seeded() {
        Ok(true) => {
            eprintln!("codex-empirica-plugin: AGENTS.md updated with empirica system prompt")
        }
        Ok(false) => {}
        Err(e) => eprintln!("codex-empirica-plugin: AGENTS.md seed failed (non-fatal): {e}"),
    }

    // Ensure empirica subagents exist in <codex_home>/agents/empirica/ so the
    // codex agent can delegate to architecture/security/ux/etc specialists.
    // Same fail-open semantics as AGENTS.md seeding.
    match subagents::ensure_subagents_seeded() {
        Ok(0) => {}
        Ok(n) => eprintln!("codex-empirica-plugin: synced {n} empirica subagent(s)"),
        Err(e) => eprintln!("codex-empirica-plugin: subagent seed failed (non-fatal): {e}"),
    }

    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("codex-empirica-plugin session-start: failed to read stdin: {e}");
        return ExitCode::SUCCESS;
    }

    // SessionStart hook commands are launched by the harness before the model
    // can request sandboxed tool execution. Use that narrow host-side boundary
    // to establish git as the practice transport, then let the canonical
    // session-init hook create the session against the initialized practice.
    match practice_bootstrap::ensure_practice(&input) {
        Ok(outcome) if outcome.changed() => {
            eprintln!(
                "codex-empirica-plugin: initialized practice at {}",
                outcome.workspace().display()
            );
        }
        Ok(_) => {}
        Err(error) => {
            eprintln!("codex-empirica-plugin: practice bootstrap failed (non-fatal): {error}")
        }
    }

    match empirica_cli::run_hook_script("session-init.py", &input) {
        Ok(output) => {
            // ecodex T81 Tx-AE: SessionStart codex schema requires
            // hookSpecificOutput.{hookEventName,additionalContext} instead
            // of CC's flat `context` field.
            print!(
                "{}",
                crate::translate_output::translate("SessionStart", &output.stdout)
            );
            eprint!("{}", output.stderr);
            match output.exit_code {
                0 => ExitCode::SUCCESS,
                2 => ExitCode::from(2),
                code => ExitCode::from((code & 0xff) as u8),
            }
        }
        Err(e) => {
            eprintln!("codex-empirica-plugin session-start: subprocess failure ({e})");
            ExitCode::SUCCESS
        }
    }
}
