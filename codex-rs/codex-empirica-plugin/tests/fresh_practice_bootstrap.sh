#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
plugin_bin=${1:-"$repo_root/codex-rs/target/debug/codex-empirica-plugin"}
sandbox_bin=${2:-"$repo_root/codex-rs/target/debug/codex-linux-sandbox"}
hooks_dir="$repo_root/codex-rs/codex-empirica-plugin/assets/hooks_scripts/hooks"
e2e_ai_id=${EMPIRICA_AI_ID:-ecodex-bootstrap-e2e}
e2e_root=$(mktemp -d "${TMPDIR:-/tmp}/ecodex-bootstrap-e2e.XXXXXX")
practice="$e2e_root/practice"
test_home="$e2e_root/home"

cleanup() {
    case "$e2e_root" in
        "${TMPDIR:-/tmp}"/ecodex-bootstrap-e2e.*) rm -rf -- "$e2e_root" ;;
        *) printf 'refusing to remove unexpected temp path: %s\n' "$e2e_root" >&2 ;;
    esac
}
trap cleanup EXIT

# The sandbox-denial check below uses the helper's exit code as evidence; a
# missing binary would "fail" identically to a real denial and prove nothing.
# Require both binaries up front so absence is loud, not a vacuous pass.
for bin in "$plugin_bin" "$sandbox_bin"; do
    if [ ! -x "$bin" ]; then
        printf 'required binary not built: %s (cargo build -p codex-empirica-plugin -p codex-linux-sandbox)\n' "$bin" >&2
        exit 1
    fi
done

mkdir -p "$practice" "$test_home"
hook_input=$(printf \
    '{"session_id":"11111111-1111-4111-8111-111111111111","cwd":"%s","type":"startup"}' \
    "$practice")

run_session_start() {
    local stdout=$1
    local stderr=$2
    (
        cd "$practice"
        printf '%s' "$hook_input" | env \
            HOME="$test_home" \
            EMPIRICA_AI_ID="$e2e_ai_id" \
            EMPIRICA_HOOKS_DIR="$hooks_dir" \
            "$plugin_bin" session-start >"$stdout" 2>"$stderr"
    )
}

run_session_start "$e2e_root/first.stdout" "$e2e_root/first.stderr"
test -d "$practice/.git"
test -f "$practice/.empirica/project.yaml"
grep -F "ai_id: \"$e2e_ai_id\"" "$practice/.empirica/project.yaml" >/dev/null
grep -F 'initialized practice at' "$e2e_root/first.stderr" >/dev/null
grep -F 'hookSpecificOutput' "$e2e_root/first.stdout" >/dev/null

run_session_start "$e2e_root/second.stdout" "$e2e_root/second.stderr"
if grep -F 'initialized practice at' "$e2e_root/second.stderr" >/dev/null; then
    echo 'second SessionStart unexpectedly reinitialized the practice' >&2
    exit 1
fi

permission_profile='{"type":"managed","file_system":{"type":"restricted","entries":[{"path":{"type":"special","value":{"kind":"root"}},"access":"read"},{"path":{"type":"special","value":{"kind":"project_roots"}},"access":"write"},{"path":{"type":"special","value":{"kind":"slash_tmp"}},"access":"write"},{"path":{"type":"special","value":{"kind":"tmpdir"}},"access":"write"},{"path":{"type":"special","value":{"kind":"project_roots","subpath":".git"}},"access":"read","missing_path_behavior":"skip"},{"path":{"type":"special","value":{"kind":"project_roots","subpath":".agents"}},"access":"read","missing_path_behavior":"skip"},{"path":{"type":"special","value":{"kind":"project_roots","subpath":".codex"}},"access":"read","missing_path_behavior":"skip"}]},"network":"restricted"}'
if (
    cd "$practice"
    "$sandbox_bin" \
        --sandbox-policy-cwd "$practice" \
        --permission-profile "$permission_profile" \
        -- git config ecodex.bootstrap-protection-broken true
); then
    echo 'sandboxed agent unexpectedly mutated protected .git metadata' >&2
    exit 1
fi
if git -C "$practice" config --get ecodex.bootstrap-protection-broken >/dev/null; then
    echo 'protected git config mutation persisted' >&2
    exit 1
fi

printf 'fresh practice bootstrap verified at %s; default sandbox kept .git protected\n' "$practice"
