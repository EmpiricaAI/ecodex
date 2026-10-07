/// The current Codex CLI version as embedded at compile time.
#[cfg(not(test))]
pub const CODEX_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Unit tests render the version into snapshots; a fixed value keeps a patch
/// release from invalidating them. It is an ordinary release number rather than
/// `0.0.0`, which the service-version notice treats as a source build.
#[cfg(test)]
pub const CODEX_CLI_VERSION: &str = "0.1.0";
