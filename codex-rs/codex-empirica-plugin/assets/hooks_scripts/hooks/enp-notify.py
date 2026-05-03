#!/usr/bin/env python3
"""
ENP Notification Hook — Surfaces pending git folder notifications.

SessionStart hook. Checks ~/.empirica/enp/pending.json for unacknowledged
notifications and injects them into the session context.

Part of ENP v0 (Epistemic Network Protocol proof of concept).
"""

import json
import sys
import time
from pathlib import Path

PENDING_PATH = Path.home() / '.empirica' / 'enp' / 'pending.json'


def format_notification(n: dict) -> str:
    """Format a single notification for Claude context."""
    authors = ', '.join(n.get('authors', ['Unknown']))
    files = n.get('files', [])
    label = n.get('label', 'Unknown')
    messages = n.get('messages', [])
    commit_count = n.get('commit_count', 0)

    # Build file summary
    if len(files) <= 5:
        file_list = '\n'.join(f'  - {f}' for f in files)
    else:
        file_list = '\n'.join(f'  - {f}' for f in files[:5])
        file_list += f'\n  ... and {len(files) - 5} more'

    # Build message summary
    msg_summary = ''
    if messages:
        msg_summary = '\n  Commits: ' + '; '.join(messages[:3])
        if len(messages) > 3:
            msg_summary += f' (+{len(messages) - 3} more)'

    return (
        f'**{label}** — {authors} pushed {commit_count} commit(s) '
        f'affecting {len(files)} watched file(s):\n'
        f'{file_list}{msg_summary}'
    )


def main():
    hook_input = json.loads(sys.stdin.read())
    event = hook_input.get('event', '')

    # Only run on SessionStart
    if event != 'SessionStart':
        print(json.dumps({"continue": True}))
        return

    if not PENDING_PATH.exists():
        print(json.dumps({"continue": True}))
        return

    try:
        pending = json.loads(PENDING_PATH.read_text())
    except (json.JSONDecodeError, OSError):
        print(json.dumps({"continue": True}))
        return

    if not isinstance(pending, list):
        print(json.dumps({"continue": True}))
        return

    # Filter to unacknowledged, unexpired (48h)
    now = time.time()
    cutoff = now - (48 * 3600)
    unacked = [
        n for n in pending
        if not n.get('acknowledged')
        and n.get('timestamp_epoch', 0) > cutoff
    ]

    if not unacked:
        print(json.dumps({"continue": True}))
        return

    # Format notifications
    lines = [f'## ENP: {len(unacked)} Pending Notification(s)\n']
    for n in unacked:
        lines.append(format_notification(n))
        lines.append('')

    lines.append(
        '*To acknowledge: tell the user about these updates. '
        'They will be cleared after 48h or when the user confirms.*'
    )

    message = '\n'.join(lines)

    result = {
        "continue": True,
        "systemMessage": message
    }
    print(json.dumps(result))


if __name__ == '__main__':
    main()
