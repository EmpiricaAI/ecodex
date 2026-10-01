use super::*;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

fn installed_plugin_dir(codex_home: &Path) -> io::Result<PathBuf> {
    Ok(codex_home
        .join("plugins/cache/empiricaAI/empirica")
        .join(manifest_version()?))
}

#[test]
fn fresh_home_gets_the_plugin_and_the_default_config() -> io::Result<()> {
    let home = TempDir::new()?;

    let report = provision(home.path())?;

    assert_eq!(
        report,
        ProvisionReport {
            plugin_written: true,
            config_created: true,
            ..ProvisionReport::default()
        }
    );
    let plugin_dir = installed_plugin_dir(home.path())?;
    assert_eq!(
        fs::read_to_string(plugin_dir.join(".codex-plugin/plugin.json"))?,
        MANIFEST
    );
    assert_eq!(fs::read_to_string(plugin_dir.join("hooks.json"))?, HOOKS);
    assert_eq!(
        fs::read_to_string(home.path().join("config.toml"))?,
        DEFAULT_CONFIG
    );
    assert_eq!(
        fs::read_to_string(home.path().join("huggingface.config.toml"))?,
        HUGGINGFACE_PROFILE
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let gate = fs::metadata(plugin_dir.join("hooks_scripts/hooks/sentinel-gate.py"))?;
        assert_eq!(gate.permissions().mode() & 0o777, 0o755);
    }

    assert_eq!(provision(home.path())?, ProvisionReport::default());
    Ok(())
}

#[test]
fn existing_config_gains_the_plugin_entry() -> io::Result<()> {
    let home = TempDir::new()?;
    let config_path = home.path().join("config.toml");
    fs::write(&config_path, "model = \"gpt-5\"\n")?;

    let report = provision(home.path())?;

    assert_eq!(
        report,
        ProvisionReport {
            plugin_written: true,
            plugin_enabled: true,
            ..ProvisionReport::default()
        }
    );
    assert_eq!(
        fs::read_to_string(&config_path)?,
        "model = \"gpt-5\"\n\n[plugins.\"empirica@empiricaAI\"]\nenabled = true\n"
    );
    Ok(())
}

#[test]
fn explicit_opt_out_is_left_alone() -> io::Result<()> {
    let home = TempDir::new()?;
    let config_path = home.path().join("config.toml");
    let config = "[plugins.\"empirica@empiricaAI\"]\nenabled = false\n";
    fs::write(&config_path, config)?;

    let report = provision(home.path())?;

    assert_eq!(
        report,
        ProvisionReport {
            plugin_written: true,
            ..ProvisionReport::default()
        }
    );
    assert_eq!(fs::read_to_string(&config_path)?, config);
    Ok(())
}

#[test]
fn legacy_entry_is_renamed_with_its_settings() -> io::Result<()> {
    let home = TempDir::new()?;
    let config_path = home.path().join("config.toml");
    fs::write(
        &config_path,
        "[plugins.\"empirica@nubaeon\"]\nenabled = true\n\n\
         [plugins.\"empirica@nubaeon\".mcp_servers.empirica.tools.finding_log]\n\
         approval_mode = \"approve\"\n",
    )?;

    let report = provision(home.path())?;

    assert_eq!(
        report,
        ProvisionReport {
            plugin_written: true,
            legacy_entry_migrated: true,
            ..ProvisionReport::default()
        }
    );
    assert_eq!(
        fs::read_to_string(&config_path)?,
        "[plugins.\"empirica@empiricaAI\"]\nenabled = true\n\n\
         [plugins.\"empirica@empiricaAI\".mcp_servers.empirica.tools.finding_log]\n\
         approval_mode = \"approve\"\n"
    );
    Ok(())
}

#[test]
fn refresh_replaces_changed_files_and_prunes_stale_ones() -> io::Result<()> {
    let home = TempDir::new()?;
    provision(home.path())?;
    let plugin_dir = installed_plugin_dir(home.path())?;
    let stale = plugin_dir.join("skills/retired-skill/SKILL.md");
    write_atomically(&stale, b"retired")?;
    write_atomically(&plugin_dir.join("hooks.json"), b"{}")?;
    let python_cache = plugin_dir.join("hooks_scripts/hooks/__pycache__/gate.cpython-314.pyc");
    write_atomically(&python_cache, b"bytecode")?;
    write_atomically(&plugin_dir.join(BUNDLE_MARKER), b"an older bundle")?;

    let report = provision(home.path())?;

    assert_eq!(
        report,
        ProvisionReport {
            plugin_written: true,
            ..ProvisionReport::default()
        }
    );
    assert_eq!(
        (
            stale.exists(),
            fs::read_to_string(plugin_dir.join("hooks.json"))?,
            python_cache.exists(),
        ),
        (false, HOOKS.to_string(), true)
    );
    Ok(())
}

#[test]
fn source_tree_leftovers_are_not_shipped() {
    assert_eq!(
        [
            "hooks/sentinel-gate.py",
            "hooks/__pycache__/sentinel-gate.cpython-314.pyc",
            "hooks/.empirica/sessions.db",
            "lib/project_resolver.pyc",
        ]
        .map(|path| is_shipped(Path::new(path))),
        [true, false, false, false]
    );
}
