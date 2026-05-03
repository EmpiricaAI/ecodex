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

    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("codex-empirica-plugin session-start: failed to read stdin: {e}");
        return ExitCode::SUCCESS;
    }

    match empirica_cli::run_hook_script("session-init.py", &input) {
        Ok(output) => {
            print!("{}", output.stdout);
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
