# Empirica and MCP

Empirica no longer ships an MCP server, and the ecodex plugin does not declare
one. The `empirica` CLI is the one integration surface: the model runs it
through its shell, which is how every Empirica operation reaches ecodex, from
`finding-log` to `check-submit`. There are no `mcp__empirica__*` tools.

Earlier ecodex builds declared the server in the plugin, switched off by
default, for a client without a shell. Such a client has no Empirica route
through MCP now; the CLI needs a shell.

## If your config still has the old block

A block like this in `~/.codex/config.toml`, written by an earlier build, does
nothing:

```toml
[plugins."empirica@empiricaAI".mcp_servers.empirica]
enabled = false
```

codex applies a plugin's MCP settings only to servers the plugin declares, and
the plugin declares none, so it never reads the block and prints no warning. You
can delete it. ecodex does not touch an existing config beyond adding the plugin
entry when it is missing, so it will not remove the block for you. A new
`config.toml` no longer gets it.

On upgrade ecodex also removes the plugin's old `mcp_servers.json` from
`~/.codex/plugins/cache/empiricaAI/empirica/`, since the new bundle does not
contain it.

## The Cortex MCP server is separate

The Cortex MCP server carries cloud knowledge sharing and the AI mesh. It is
configured under `[mcp_servers.cortex]` and is unchanged; see
[`cross-ai-mesh.md`](../cross-ai-mesh.md).
