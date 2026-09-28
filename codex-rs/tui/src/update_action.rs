#[cfg(any(not(debug_assertions), test))]
use codex_install_context::InstallContext;
#[cfg(any(not(debug_assertions), test))]
use codex_install_context::InstallMethod;
#[cfg(any(not(debug_assertions), test))]
use codex_install_context::StandalonePlatform;
#[cfg(any(not(debug_assertions), test))]
use std::path::Path;

/// Binary that `scripts/install.sh` installs next to `ecodex`. Its presence in
/// the executable's directory identifies an install-script layout.
#[cfg(any(not(debug_assertions), test))]
const INSTALL_SCRIPT_SIBLING: &str = "codex-empirica-plugin";

/// Environment variable `scripts/install.sh` reads for its install directory.
pub const INSTALL_DIR_ENV_VAR: &str = "ECODEX_INSTALL_DIR";

/// Update action the CLI should perform after the TUI exits.
///
/// ecodex: only channels ecodex actually distributes through are represented
/// here. npm/bun/pnpm and the Windows standalone installer have no ecodex
/// distribution (see docs/ecodex/INSTALL.md — Windows isn't supported yet),
/// so `from_install_context` returns `None` for those methods rather than
/// offering a command that would install/update the wrong product.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateAction {
    /// Replace the local daemon after restoring the terminal.
    Daemon(DaemonUpdateSource),
    /// Update via `brew upgrade EmpiricaAI/tap/ecodex`.
    BrewUpgrade,
    /// Update via `cargo install --git https://github.com/EmpiricaAI/ecodex codex-cli`.
    CargoInstall,
    /// Update via `curl -fsSL .../scripts/install.sh | bash` (re-run, idempotent).
    /// The CLI points the script at the running binary's directory through
    /// [`INSTALL_DIR_ENV_VAR`], so a custom `--prefix` install updates in place.
    StandaloneUnix,
}

impl UpdateAction {
    #[cfg(any(not(debug_assertions), test))]
    pub(crate) fn from_install_context(context: &InstallContext) -> Option<Self> {
        match &context.method {
            InstallMethod::Brew => Some(UpdateAction::BrewUpgrade),
            InstallMethod::Standalone {
                platform: StandalonePlatform::Unix,
                ..
            } => Some(UpdateAction::StandaloneUnix),
            // ecodex ships no npm/bun/pnpm package and no Windows standalone
            // installer yet -- offering these would run an update command for
            // a different product (or one that doesn't exist for ecodex).
            InstallMethod::Npm
            | InstallMethod::Bun
            | InstallMethod::VitePlus
            | InstallMethod::Pnpm
            | InstallMethod::Standalone {
                platform: StandalonePlatform::Windows,
                ..
            }
            | InstallMethod::Other => None,
        }
    }

    /// Detects ecodex's own install channels from the executable's location.
    ///
    /// The shared install context only recognises Homebrew on macOS and
    /// upstream's `~/.codex/packages` layout, so it reports `Other` for the
    /// channels ecodex ships through on Linux: linuxbrew, cargo and the
    /// install script.
    #[cfg(any(not(debug_assertions), test))]
    pub(crate) fn from_exe(exe: &Path, cargo_home: Option<&Path>) -> Option<Self> {
        let exe_dir = exe.parent()?;
        if exe.components().any(|part| part.as_os_str() == "Cellar") {
            Some(UpdateAction::BrewUpgrade)
        } else if cargo_home.is_some_and(|home| exe_dir == home.join("bin")) {
            Some(UpdateAction::CargoInstall)
        } else if !cfg!(windows) && exe_dir.join(INSTALL_SCRIPT_SIBLING).is_file() {
            Some(UpdateAction::StandaloneUnix)
        } else {
            None
        }
    }

    /// Returns the list of command-line arguments for invoking the update.
    pub fn command_args(self) -> (&'static str, &'static [&'static str]) {
        match self {
            // Display only: run_update_action executes the daemon update through the
            // launching CLI's own path, not this program name.
            UpdateAction::Daemon(source) => ("ecodex", source.command_args()),
            UpdateAction::BrewUpgrade => ("brew", &["upgrade", "EmpiricaAI/tap/ecodex"]),
            UpdateAction::CargoInstall => (
                "cargo",
                &[
                    "install",
                    "--git",
                    "https://github.com/EmpiricaAI/ecodex",
                    "codex-cli",
                ],
            ),
            UpdateAction::StandaloneUnix => (
                "sh",
                &[
                    "-c",
                    "curl -fsSL https://raw.githubusercontent.com/EmpiricaAI/ecodex/main/scripts/install.sh | bash",
                ],
            ),
        }
    }

    /// Returns string representation of the command-line arguments for invoking the update.
    pub fn command_str(self) -> String {
        let (command, args) = self.command_args();
        shlex::try_join(std::iter::once(command).chain(args.iter().copied()))
            .unwrap_or_else(|_| format!("{command} {}", args.join(" ")))
    }
}

#[cfg(not(debug_assertions))]
pub fn get_update_action() -> Option<UpdateAction> {
    UpdateAction::from_install_context(InstallContext::current()).or_else(|| {
        let exe = std::env::current_exe().ok()?;
        let cargo_home = std::env::var_os("CARGO_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join(".cargo")));
        UpdateAction::from_exe(&exe, cargo_home.as_deref())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;

    #[test]
    fn maps_install_context_to_update_action() {
        let native_release_dir =
            AbsolutePathBuf::from_absolute_path(std::env::temp_dir().join("native-release"))
                .expect("temp dir path should be absolute");

        assert_eq!(
            UpdateAction::from_install_context(&InstallContext {
                method: InstallMethod::Other,
                package_layout: None,
            }),
            None
        );
        // ecodex ships no npm/bun/pnpm package, so these have no update action.
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext {
                method: InstallMethod::Npm,
                package_layout: None,
            }),
            None
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext {
                method: InstallMethod::Bun,
                package_layout: None,
            }),
            None
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext {
                method: InstallMethod::VitePlus,
                package_layout: None,
            }),
            None
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext {
                method: InstallMethod::Pnpm,
                package_layout: None,
            }),
            None
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext {
                method: InstallMethod::Brew,
                package_layout: None,
            }),
            Some(UpdateAction::BrewUpgrade)
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext {
                method: InstallMethod::Standalone {
                    platform: StandalonePlatform::Unix,
                    release_dir: native_release_dir.clone(),
                    resources_dir: Some(native_release_dir.join("codex-resources")),
                },
                package_layout: None,
            }),
            Some(UpdateAction::StandaloneUnix)
        );
        // ecodex isn't distributed for Windows yet (docs/ecodex/INSTALL.md).
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext {
                method: InstallMethod::Standalone {
                    platform: StandalonePlatform::Windows,
                    release_dir: native_release_dir.clone(),
                    resources_dir: Some(native_release_dir.join("codex-resources")),
                },
                package_layout: None,
            }),
            None
        );
    }

    #[test]
    fn detects_ecodex_install_channels_from_exe_path() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let script_dir = root.path().join("bin");
        std::fs::create_dir_all(&script_dir)?;
        std::fs::write(script_dir.join(INSTALL_SCRIPT_SIBLING), "")?;
        let cargo_home = root.path().join(".cargo");
        let linuxbrew_exe = root
            .path()
            .join("linuxbrew/.linuxbrew/Cellar/ecodex/0.157.0/bin/ecodex");

        assert_eq!(
            [
                UpdateAction::from_exe(&linuxbrew_exe, Some(&cargo_home)),
                UpdateAction::from_exe(&cargo_home.join("bin/ecodex"), Some(&cargo_home)),
                UpdateAction::from_exe(&script_dir.join("ecodex"), Some(&cargo_home)),
                UpdateAction::from_exe(&root.path().join("bare/ecodex"), Some(&cargo_home)),
            ],
            [
                Some(UpdateAction::BrewUpgrade),
                Some(UpdateAction::CargoInstall),
                (!cfg!(windows)).then_some(UpdateAction::StandaloneUnix),
                None,
            ]
        );
        Ok(())
    }

    #[test]
    fn standalone_update_command_reruns_ecodex_installer() {
        assert_eq!(
            UpdateAction::StandaloneUnix.command_args(),
            (
                "sh",
                &[
                    "-c",
                    "curl -fsSL https://raw.githubusercontent.com/EmpiricaAI/ecodex/main/scripts/install.sh | bash"
                ][..],
            )
        );
    }

    #[test]
    fn brew_update_command_targets_ecodex_tap() {
        assert_eq!(
            UpdateAction::BrewUpgrade.command_args(),
            ("brew", &["upgrade", "EmpiricaAI/tap/ecodex"][..])
        );
    }
}

/// Package source explicitly selected by the user in the daemon menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonUpdateSource {
    PublicStable,
    ThisCli,
}

impl DaemonUpdateSource {
    pub fn command_args(self) -> &'static [&'static str] {
        match self {
            Self::PublicStable => &["app-server", "daemon", "update"],
            Self::ThisCli => &["app-server", "daemon", "update", "--from-cli", "--yes"],
        }
    }
}
