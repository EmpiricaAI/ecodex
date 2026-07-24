# codex-empirica-plugin — MCP Server

The plugin registers Empirica's MCP server with codex, exposing all `mcp__empirica__*` tools to the agent.

## Registration

`manifest.json` references `mcp_servers.json`:

```json
{
  "mcpServers": "./mcp_servers.json"
}
```

`mcp_servers.json` contents:

```json
{
  "mcp_servers": {
    "empirica": {
      "command": "empirica-mcp",
      "args": [],
      "enabled": true,
      "startup_timeout_sec": 30,
      "tool_timeout_sec": 60
    }
  }
}
```

The schema is codex's `McpServerConfig` (`codex-rs/config/src/mcp_types.rs:157`). Stdio transport with `command`/`args`. Codex spawns the subprocess on session start, communicates via MCP's stdio JSON-RPC protocol, and registers all advertised tools under the `mcp__empirica__*` namespace.

## Tools exposed

The exact tool set depends on the empirica MCP server build, but typically includes:

| Tool | Purpose |
|---|---|
| `mcp__empirica__assess_state` | Snapshot current epistemic vectors |
| `mcp__empirica__finding_log` | Log a finding artifact |
| `mcp__empirica__decision_log` | Log a decision artifact |
| `mcp__empirica__unknown_log` | Log an open question |
| `mcp__empirica__deadend_log` | Log an approach that failed |
| `mcp__empirica__mistake_log` | Log an error with prevention |
| `mcp__empirica__assumption_log` | Log an unverified belief |
| `mcp__empirica__goals_create` | Create a project goal |
| `mcp__empirica__goals_list` | List active goals |
| `mcp__empirica__goals_complete` | Close a goal |
| `mcp__empirica__project_search` | Semantic search across project history |
| `mcp__empirica__investigate` | Query knowledge base |
| `mcp__empirica__submit_preflight_assessment` | Open a transaction |
| `mcp__empirica__submit_check_assessment` | Gate noetic→praxic |
| `mcp__empirica__submit_postflight_assessment` | Close a transaction |
| ...plus many more | Full reference: `empirica mcp --help` |

These mirror the `empirica` CLI subcommands and provide an MCP-protocol-friendly interface to the same functionality. Codex agents that prefer structured tool calls over CLI invocations will naturally reach for these.

## Verification

After install:
```sh
ecodex mcp list                      # confirm 'empirica' is in the registered server list
ecodex --tool-list | grep empirica   # confirm tools are registered
```

## Launcher binary

Empirica's MCP server is launched by the dedicated `empirica-mcp` binary (no arguments) — not a subcommand of the `empirica` CLI. This is what the vendored `mcp_servers.json` ships:

```json
{
  "mcp_servers": {
    "empirica": {
      "command": "empirica-mcp",
      "args": []
    }
  }
}
```

## Configuration

| Variable | Default | Purpose |
|---|---|---|
| `startup_timeout_sec` | 30 | How long codex waits for MCP server to register tools on startup |
| `tool_timeout_sec` | 60 | Default timeout for individual tool calls |

If empirica's startup is slower (cold Qdrant connection, etc.), bump `startup_timeout_sec` to 60 or higher.
