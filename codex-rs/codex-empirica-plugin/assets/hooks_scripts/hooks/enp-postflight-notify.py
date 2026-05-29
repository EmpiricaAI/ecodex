#!/usr/bin/env python3
"""
ENP PostToolUse Hook — Surfaces new notifications after POSTFLIGHT.

Fires on Bash PostToolUse, checks if the command was postflight-submit.
If so, checks for a flag file written by the cron watcher.
Notifications surface at transaction boundaries, not mid-work.
"""

import json
import sys
import time
from pathlib import Path

ENP_DIR = Path.home() / '.empirica' / 'enp'
FLAG_PATH = ENP_DIR / 'new_notification_flag'
PENDING_PATH = ENP_DIR / 'pending.json'


def format_notification(n: dict) -> str:
    authors = ', '.join(n.get('authors', ['Unknown']))
    files = n.get('files', [])
    label = n.get('label', 'Unknown')
    messages = n.get('messages', [])
    commit_count = n.get('commit_count', 0)

    if len(files) <= 5:
        file_list = ', '.join(files)
    else:
        file_list = ', '.join(files[:3]) + f' +{len(files) - 3} more'

    msg_hint = ''
    if messages:
        msg_hint = f' — "{messages[0]}"'
        if len(messages) > 1:
            msg_hint += f' +{len(messages) - 1} more'

    return (
        f'**{label}**: {authors} pushed {commit_count} commit(s) '
        f'touching {len(files)} file(s): {file_list}{msg_hint}'
    )


def main():
    hook_input = json.loads(sys.stdin.read()) if not sys.stdin.isatty() else {}

    # Only fire after postflight-submit completes
    tool_input = hook_input.get('tool_input', {})
    command = tool_input.get('command', '')
    if 'postflight-submit' not in command:
        print(json.dumps({"continue": True}))
        return

    # Fast path: no flag = no work
    if not FLAG_PATH.exists():
        print(json.dumps({"continue": True}))
        return

    # Flag exists — read notifications
    try:
        FLAG_PATH.unlink()  # Clear flag immediately (atomic-ish)
    except OSError:
        pass

    try:
        pending = json.loads(PENDING_PATH.read_text())
    except (json.JSONDecodeError, OSError, FileNotFoundError):
        print(json.dumps({"continue": True}))
        return

    # Filter to unacknowledged
    now = time.time()
    cutoff = now - (48 * 3600)
    unacked = [
        n for n in pending
        if not n.get('acknowledged')
        and n.get('timestamp_epoch', 0) > cutoff
        and not n.get('surfaced_in_session')
    ]

    if not unacked:
        print(json.dumps({"continue": True}))
        return

    # Mark as surfaced so we don't repeat
    for n in pending:
        if not n.get('acknowledged') and not n.get('surfaced_in_session'):
            n['surfaced_in_session'] = True

    PENDING_PATH.write_text(json.dumps(pending, indent=2))

    # Format message
    lines = [f'ENP: {len(unacked)} new update(s) detected:']
    for n in unacked:
        lines.append(f'  - {format_notification(n)}')
    lines.append('Surface this to the user and ask if they want to pull/review.')

    result = {
        "continue": True,
        "systemMessage": '\n'.join(lines)
    }
    print(json.dumps(result))


if __name__ == '__main__':
    main()
