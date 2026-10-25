//! ecodex: plugin writable roots must not cost the session its permission
//! profile identity.

use super::plugin_enriched_permission_snapshot;
use codex_protocol::models::ActivePermissionProfile;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::PermissionProfileSnapshot;
use codex_protocol::models::ProfileWorkspaceRoot;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

fn workspace_root() -> ProfileWorkspaceRoot {
    let root =
        AbsolutePathBuf::from_absolute_path(std::env::temp_dir()).expect("temp dir is absolute");
    ProfileWorkspaceRoot::from(root)
}

#[test]
fn enrichment_keeps_the_active_profile_and_its_workspace_roots() {
    let current = PermissionProfileSnapshot::active_with_profile_workspace_roots(
        PermissionProfile::workspace_write(),
        ActivePermissionProfile::new(":workspace"),
        vec![workspace_root()],
    );
    let enriched = PermissionProfile::read_only();

    let snapshot = plugin_enriched_permission_snapshot(&current, enriched.clone());

    assert_eq!(
        snapshot,
        PermissionProfileSnapshot::active_with_profile_workspace_roots(
            enriched,
            ActivePermissionProfile::new(":workspace"),
            vec![workspace_root()],
        )
    );
}

#[test]
fn enrichment_of_a_legacy_profile_stays_legacy() {
    let current = PermissionProfileSnapshot::legacy(PermissionProfile::workspace_write());
    let enriched = PermissionProfile::read_only();

    let snapshot = plugin_enriched_permission_snapshot(&current, enriched.clone());

    assert_eq!(snapshot, PermissionProfileSnapshot::legacy(enriched));
}
