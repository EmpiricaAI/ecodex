# Empirica's MCP server

The empirica plugin declares Empirica's MCP server, `empirica-mcp`. It exposes
the `empirica` CLI's operations to the model as `mcp__empirica__*` tools.

**It is off by default.** The server only wraps the `empirica` CLI, and wherever
ecodex has a shell the model runs `empirica` itself, so the server adds nothing
there. Turn it on for a client that has no shell, such as a GUI or web front
end.

This is separate from the Cortex MCP server, which carries cloud knowledge
sharing and the AI mesh. Its default does not change.

## Turning it on or off

The switch lives in `~/.codex/config.toml`:

```toml
[plugins."empirica@empiricaAI".mcp_servers.empirica]
enabled = false   # true to turn the server on
```

ecodex writes this block, set to `false`, into a new `config.toml` and into an
existing one that has no entry for the plugin yet. A plugin entry you already
have is left as it is; add the block to it to turn the server off.

Check the result with:

```sh
ecodex mcp list
```

It lists `empirica` with its enabled state.

## How the server is declared

The plugin's `manifest.json` points at `mcp_servers.json`:

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

codex starts `empirica-mcp` over stdio and registers what it advertises under
`mcp__empirica__*`. The manifest declares the server enabled on purpose: your
config can turn a plugin's MCP server off but never back on, so the default has
to live in config, where you can flip it either way. For the same reason the
timeouts are the manifest's; config controls a plugin server's enablement and
tool policy, not how it is launched.

## Tools

Each tool is one `empirica` CLI verb, named with underscores: `finding_log` runs
`finding-log`, `goals_create` runs `goals-create`, and so on. The current list,
with the verb each tool routes to:

```sh
empirica mcp-list-tools
```
