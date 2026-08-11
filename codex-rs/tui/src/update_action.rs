#[cfg(any(not(debug_assertions), test))]
use codex_install_context::InstallContext;
#[cfg(any(not(debug_assertions), test))]
use codex_install_context::InstallMethod;
#[cfg(any(not(debug_assertions), test))]
use codex_install_context::StandalonePlatform;

/// Update action the CLI should perform after the TUI exits.
///
/// ecodex: only channels ecodex actually distributes through are represented
/// here. npm/bun/pnpm and the Windows standalone installer have no ecodex
/// distribution (see docs/ecodex/INSTALL.md — Windows isn't supported yet),
/// so `from_install_context` returns `None` for those methods rather than
/// offering a command that would install/update the wrong product.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateAction {
    /// Update via `brew upgrade EmpiricaAI/tap/ecodex`.
    BrewUpgrade,
    /// Update via `curl -fsSL .../scripts/install.sh | bash` (re-run, idempotent).
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
            | InstallMethod::Pnpm
            | InstallMethod::Standalone {
                platform: StandalonePlatform::Windows,
                ..
            }
            | InstallMethod::Other => None,
        }
    }

    /// Returns the list of command-line arguments for invoking the update.
    pub fn command_args(self) -> (&'static str, &'static [&'static str]) {
        match self {
            UpdateAction::BrewUpgrade => ("brew", &["upgrade", "EmpiricaAI/tap/ecodex"]),
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
    UpdateAction::from_install_context(InstallContext::current())
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
