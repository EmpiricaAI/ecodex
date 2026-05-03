#!/usr/bin/env python3
"""
Empirica PostToolUseFailure Hook — Auto-log dead-ends from tool failures.

Fires when a tool call fails. Logs the failure as a dead-end to prevent
re-exploration of failed approaches. Tracks failure patterns.

Input: tool_name, tool_input, error, is_interrupt
Can block: No (tool already failed)
"""

import json
import logging
import subprocess
import sys
from pathlib import Path

LOG_DIR = Path.home() / '.empirica' / 'logs'
LOG_DIR.mkdir(parents=True, exist_ok=True)
logger = logging.getLogger('empirica.tool-failure')
handler = logging.FileHandler(LOG_DIR / 'tool-failure.log')
handler.setFormatter(logging.Formatter('%(asctime)s %(message)s'))
logger.addHandler(handler)
logger.setLevel(logging.DEBUG)

# Tool failures that are noise (not worth logging as dead-ends)
IGNORE_TOOLS = {
    'Read',       # File not found is normal exploration
    'Glob',       # No matches is normal
    'Grep',       # No matches is normal
    'LSP',        # LSP failures are common and transient
}

# Error patterns that are noise
IGNORE_PATTERNS = [
    'No such file or directory',
    'No matches found',
    'not a tty',
    'Permission denied',  # Usually sandbox, not a real dead-end
]


def _is_interesting_failure(tool_name: str, error: str) -> bool:
    """Determine if this failure is worth logging as a dead-end."""
    if tool_name in IGNORE_TOOLS:
        return False
    for pattern in IGNORE_PATTERNS:
        if pattern in error:
            return False
    # Short errors are usually transient
    return len(error) >= 20


def _truncate(s: str, max_len: int = 200) -> str:
    """Truncate string for logging."""
    return s[:max_len] + '...' if len(s) > max_len else s


def main():
    try:
        hook_input = json.loads(sys.stdin.read())
    except (json.JSONDecodeError, EOFError):
        hook_input = {}

    tool_name = hook_input.get('tool_name', 'unknown')
    tool_input = hook_input.get('tool_input', {})
    error = hook_input.get('error', '')
    is_interrupt = hook_input.get('is_interrupt', False)

    logger.info(f"ToolFailure: {tool_name} | interrupt={is_interrupt} | {_truncate(error, 100)}")

    # Skip interrupts — user cancelled, not a real failure
    if is_interrupt:
        logger.debug("  Skipping interrupt")
        sys.exit(0)

    # Skip uninteresting failures
    if not _is_interesting_failure(tool_name, error):
        logger.debug(f"  Skipping noise failure for {tool_name}")
        sys.exit(0)

    # Build a meaningful description of what failed
    if tool_name == 'Bash':
        command = tool_input.get('command', 'unknown command')
        approach = f"Bash: {_truncate(command, 150)}"
    elif tool_name in ('Edit', 'Write'):
        file_path = tool_input.get('file_path', 'unknown file')
        approach = f"{tool_name}: {file_path}"
    else:
        approach = f"{tool_name}: {_truncate(str(tool_input), 150)}"

    why_failed = _truncate(error, 300)

    # Log as dead-end
    try:
        result = subprocess.run(
            ['empirica', 'deadend-log',
             '--approach', approach,
             '--why-failed', why_failed],
            capture_output=True, text=True, timeout=5
        )
        if result.returncode == 0:
            logger.info(f"  Logged dead-end: {approach}")
        else:
            logger.warning(f"  Failed to log dead-end: {result.stderr}")
    except Exception as e:
        logger.warning(f"  Exception logging dead-end: {e}")

    sys.exit(0)


if __name__ == '__main__':
    main()
