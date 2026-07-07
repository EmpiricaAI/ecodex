//! Generic hook dispatcher — runs an arbitrary vendored Empirica script
//! against a named codex hook event.
//!
//! Enables multi-script fan-out per hook event without adding a dedicated
//! Rust handler module for each script. CC's `~/.claude/settings.json`
//! wires N scripts to a single event (e.g. UserPromptSubmit fires
//! tool-router + context-shift-tracker + 4 install/uninstall pickups);
//! this dispatcher lets ecodex's `hooks.json` declare the same fan-out
//! by listing multiple `codex-empirica-plugin run-hook EVENT SCRIPT.py`
//! commands per matcher group.
//!
//! The primary per-event handlers (sentinel-gate via pre-tool-use,
//! tool-router via user-prompt-submit, etc.) stay first in the fan-out
//! order to preserve the canonical Empirica entry point; sibling scripts
//! follow via this dispatcher.
//!
//! Same fail-open semantics as the typed handlers.

use std::io::Read;
use std::process::ExitCode;

use crate::empirica_cli;
use crate::translate_output;

/// Run an arbitrary Empirica hook script for a named codex hook event.
///
/// Arguments are the codex hook event name (e.g. `UserPromptSubmit`) and
/// the bare script filename (e.g. `context-shift-tracker.py`) — the
/// script is resolved against the plugin's vendored
/// `hooks_scripts/hooks/` tree by `empirica_cli`.
pub fn run(event_name: &str, script: &str) -> ExitCode {
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!(
            "codex-empirica-plugin run-hook {event_name} {script}: failed to read stdin: {e}"
        );
        return ExitCode::SUCCESS;
    }

    match empirica_cli::run_hook_script(script, &input) {
        Ok(output) => {
            print!(
                "{}",
                translate_output::translate(event_name, &output.stdout)
            );
            eprint!("{}", output.stderr);
            match output.exit_code {
                0 => ExitCode::SUCCESS,
                2 => ExitCode::from(2),
                code => ExitCode::from((code & 0xff) as u8),
            }
        }
        Err(e) => {
            eprintln!(
                "codex-empirica-plugin run-hook {event_name} {script}: subprocess failure ({e})"
            );
            ExitCode::SUCCESS
        }
    }
}
