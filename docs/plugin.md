# The agent plugin

`plugins/distill` is one plugin that installs in both Codex and Claude Code. It has three
parts: a hook that runs on every prompt, an MCP server with four tools, and the `distill` skill
that tells the agent when to use them. Installing it: [INSTALL.md](../INSTALL.md).

## Files

| File | Read by | Purpose |
|---|---|---|
| `.claude-plugin/plugin.json` | Claude Code | Manifest; declares the MCP server inline |
| `.codex-plugin/plugin.json` | Codex | Manifest; points at `skills/` and `codex.mcp.json` |
| `codex.mcp.json` | Codex | MCP server, started from the plugin directory (`cwd: "."`) |
| `hooks/hooks.json` | both | `UserPromptSubmit` hook. Both agents expand `${CLAUDE_PLUGIN_ROOT}` |
| `bin/distill-launch` | both | Finds and runs the `distill` binary |
| `skills/distill/SKILL.md` | both | Every rule the agent follows |

The marketplaces that list the plugin are `.claude-plugin/marketplace.json` and
`.agents/plugins/marketplace.json` at the repo root. Both are named `distill`, so the plugin
id is `distill@distill` in each agent.

## Finding the binary

Desktop apps often start hooks and MCP servers with a PATH that lacks the install directory.
`bin/distill-launch` tries, in order: `$DISTILL_BIN`, the `bin_path` that `distill init`
recorded in the config, `PATH`, then `~/.cargo/bin`, `/opt/homebrew/bin`, `/usr/local/bin`
and `~/.local/bin`. When nothing is found, the hook exits 0 silently and the MCP server exits
127 with a message.

The launcher is a POSIX shell script. Windows is not handled yet.

## The hook

`distill hook user-prompt-submit` reads the hook JSON on stdin and prints
`hookSpecificOutput.additionalContext`. It reads only the config file, never the vault or
index, and always exits 0.

```
distill-source: codex 01a0d530-42ae-7731-8a1d-b4e07d9b837d /Users/me/project
distill-suggest: on
Distill is installed. If you have not read the distill skill in this session, read it now …
```

The agent is Codex when the input has `turn_id` (a Codex extension), Claude Code when
`CLAUDECODE=1` is set, and otherwise is guessed from `transcript_path`. Without a known
agent, session id and cwd, the `distill-source` line is left out. `distill-suggest` follows
the `suggest.enabled` setting. Without a config, the hook injects one line asking the user to
run `distill init`.

## The MCP server

`distill mcp` serves on stdio. Every call opens the config, vault and index afresh.

| Tool | Returns |
|---|---|
| `distill_recall` | Matching topics with `ask_count`, their notes (with `file`, annotations and `reopen`), and every tag in use |
| `distill_save` | The save result (`note_id`, `topic_id`, `ask_count`, `tags`, `file`) plus `warnings` |
| `distill_search` | Notes matching keywords and/or a tag, with `file` |
| `distill_stats` | The same counts as `distill stats --json` |

A failed validation comes back as a tool error whose text says what to change. Before saving,
the server checks the source: a Codex session id must match a rollout file under
`$CODEX_HOME/sessions` or `archived_sessions` (default `~/.codex`); a Claude Code session id
that differs from the server's `CLAUDE_CODE_SESSION_ID` is kept and logged as a warning,
because the id changes after `/clear` while the server keeps running.

`reopen` holds `link` (`codex://threads/<id>` or `claude://resume?session=<id>`) and
`command` (`codex resume <id>`, or `cd <cwd> && claude --resume <id>`).

## `distill doctor` and the plugin

`distill doctor` reports the plugin as installed when `~/.claude/plugins/installed_plugins.json`
(or `$CLAUDE_CONFIG_DIR`) lists `distill@…`, or `~/.codex/config.toml` (or `$CODEX_HOME`) has an
enabled `plugins."distill@…"` table. It is a problem when neither agent has it.
