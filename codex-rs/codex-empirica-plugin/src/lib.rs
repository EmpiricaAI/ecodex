//! Library half of `codex-empirica-plugin`: the empirica plugin compiled into
//! ecodex, and the provisioner that installs it.
//!
//! The prebuilt channels (install script, Homebrew, release tarball) and
//! `cargo install` put binaries on disk and nothing else. Calling [`provision`]
//! at startup is what turns any of those installs into the integrated harness
//! rather than plain codex.

mod provision;

pub use provision::ProvisionReport;
pub use provision::provision;
