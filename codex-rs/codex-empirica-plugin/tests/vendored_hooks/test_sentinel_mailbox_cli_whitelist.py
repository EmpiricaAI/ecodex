"""sentinel-gate.py — `empirica mailbox` CLI subcommands are on the tiered whitelist.

The receive-side wake path directs a mesh-woken practitioner to run
`empirica mailbox poll --ai-id <id>` as its FIRST action (see core/ntfy_listener.rs
build_wake_item). That fires on an IDLE session with NO open transaction, so it flows
through sentinel-gate.py `_handle_no_preflight`, which allows only `is_safe_bash_command`
/ `is_transition_command`. `is_safe_bash_command` delegates to `is_safe_empirica_command`,
so `empirica mailbox poll` MUST be whitelisted or the woken practitioner is denied
"No open transaction" — the exact last-mile the wake fix was meant to close.

Read subcommands (poll/show) → Tier 1 (always allowed). Emit/mutate subcommands
(reply/archive) → Tier 2 (state-changing but part of the mesh workflow). Both tiers
satisfy is_safe_empirica_command, so all four flow pre-transaction; the tier split
preserves the read-vs-mutation semantics that mirrors the rest of the whitelist.
"""
from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

_HOOK = (
    Path(__file__).resolve().parents[2]
    / "assets"
    / "hooks_scripts"
    / "hooks"
    / "sentinel-gate.py"
)
_spec = importlib.util.spec_from_file_location("sentinel_gate_cli_mod", _HOOK)
assert _spec is not None and _spec.loader is not None
sg = importlib.util.module_from_spec(_spec)
sys.modules["sentinel_gate_cli_mod"] = sg
_spec.loader.exec_module(sg)


def test_mailbox_reads_are_tier1():
    # Pure reads — must be Tier 1 (allowed in ANY phase, incl. pre-transaction).
    for cmd in (
        "empirica mailbox poll --ai-id empirica.david.ecodex-lab --output json",
        "empirica mailbox show prop_abc123",
    ):
        assert any(cmd.startswith(p) for p in sg.EMPIRICA_TIER1_PREFIXES), cmd


def test_mailbox_mutations_are_tier2():
    # Emit/soft-mutate — Tier 2 (state-changing, allowed as part of the workflow),
    # NOT Tier 1.
    for cmd in (
        "empirica mailbox reply --parent-id prop_x --summary y --commit-sha abc",
        "empirica mailbox archive prop_x",
    ):
        assert not any(cmd.startswith(p) for p in sg.EMPIRICA_TIER1_PREFIXES), cmd
        assert any(cmd.startswith(p) for p in sg.EMPIRICA_TIER2_PREFIXES), cmd


def test_all_mailbox_subcommands_are_safe_empirica_commands():
    for cmd in (
        "empirica mailbox poll --ai-id empirica.david.ecodex-lab --output json",
        "empirica mailbox show prop_abc123",
        "empirica mailbox reply --parent-id prop_x --summary y",
        "empirica mailbox archive prop_x",
    ):
        assert sg.is_safe_empirica_command(cmd), cmd


def test_wake_first_action_poll_allowed_pre_transaction():
    # The load-bearing regression: the woken-idle FIRST action must pass
    # is_safe_bash_command so _handle_no_preflight ALLOWS it (no PREFLIGHT yet).
    poll = "empirica mailbox poll --ai-id empirica.david.ecodex-lab --output json"
    assert sg.is_safe_bash_command({"command": poll}) is True


def test_reply_is_not_ungated_send_but_still_flows():
    # reply emits a REFLEX collab_brief (not an ECO-gated proposal) — it should
    # flow through the Sentinel (is_safe) even though it's Tier 2.
    reply = "empirica mailbox reply --parent-id prop_x --summary done --commit-sha deadbeef"
    assert sg.is_safe_bash_command({"command": reply}) is True
