//! codex-empirica-plugin
//!
//! Codex hook-event dispatcher that delegates to the Empirica Python hook
//! scripts via subprocess. Each codex hook event maps to a subcommand
//! (`pre-tool-use`, `post-tool-use`, `session-start`, `user-prompt-submit`,
//! `stop`, `permission-request`).
//!
//! The subprocess boundary lives in `empirica_cli` — that single module is the
//! replacement target for future optimization (PyO3, sidecar daemon, or native
//! port). Hook handlers themselves stay stable across that swap.

use std::process::ExitCode;

mod empirica_cli;
mod hooks;

const USAGE: &str = "\
codex-empirica-plugin <hook-event>

Supported events:
  pre-tool-use         Sentinel firewall (gates praxic tools on PREFLIGHT/CHECK)
  post-tool-use        Capture tool result, update vectors
  session-start        Bootstrap empirica session, load skills
  user-prompt-submit   Inject context, detect hedges, EWM protocol
  stop                 Transaction enforcer (POSTFLIGHT before session end)
  permission-request   (codex-specific) — currently no-op
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let Some(event) = args.get(1) else {
        eprintln!("{USAGE}");
        return ExitCode::from(64);
    };

    match event.as_str() {
        "pre-tool-use" => hooks::pre_tool_use::handle(),
        "stop" => hooks::stop::handle(),
        "post-tool-use"
        | "session-start"
        | "user-prompt-submit"
        | "permission-request" => {
            // v1 stubs — succeed silently until ported.
            ExitCode::SUCCESS
        }
        unknown => {
            eprintln!("codex-empirica-plugin: unknown hook event '{unknown}'");
            eprintln!("{USAGE}");
            ExitCode::from(64)
        }
    }
}
