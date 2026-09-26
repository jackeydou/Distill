# The agent plugin

`plugins/distill` is the source of one plugin that installs in both Codex and Claude Code. It
has three parts: a hook that runs on every prompt, an MCP server with four tools, and the
`distill` skill that tells the agent when to use them. Agents install a per-agent build from a
marketplace branch, not the source directory. Installing it: [INSTALL.md](../INSTALL.md).

## Source layout

| Path | Goes to | Purpose |
|---|---|---|
| `plugins/distill/bin/distill-launch` | both | Finds and runs the `distill` binary |
| `plugins/distill/skills/distill/` | both | Every rule the agent follows |
| `plugins/distill/claude-code/` | Claude Code | `.claude-plugin/plugin.json`, `.mcp.json`, `hooks/hooks.json`, using `${CLAUDE_PLUGIN_ROOT}` |
| `plugins/distill/codex/` | Codex | `.codex-plugin/plugin.json`, `.mcp.json` (started with `cwd: "."`), `hooks/hooks.json`, using `${PLUGIN_ROOT}` |
| `packaging/claude-code/` | Claude Code | `.claude-plugin/marketplace.json` |
| `packaging/codex/` | Codex | `.agents/plugins/marketplace.json` |

Each agent reads its own native file names, and each build holds one agent's files, so the two
sets never meet in one directory. Both marketplaces are named `distill`, so the plugin id is
`distill@distill` in each agent.

## Build and publish

`scripts/build-plugins.sh` (`mise run build:plugins`) writes one marketplace per agent to
`dist/plugins/<agent>`: the marketplace manifest from `packaging/<agent>/` at the root, and the
plugin at `plugins/distill` made of `bin/`, `skills/` and the contents of
`plugins/distill/<agent>/`. Pass `claude-code` or `codex` to build one; `DISTILL_PLUGIN_OUT`
changes the output root. `--build <id>` sets the output version to `<version>+<agent>.<id>`
and needs `jq`. The `distill` binary is not included; it ships separately.

On every push to `main`, [CI](../.github/workflows/ci.yml) runs `mise run check`, then builds
each agent and commits the output to its branch:

| Branch | Holds |
|---|---|
| `marketplace-codex` | `dist/plugins/codex` |
| `marketplace-claude` | `dist/plugins/claude-code` |

The build id is the first 12 characters of the last commit that touched `plugins/distill`,
`packaging` or the build script. The published version changes only when the plugin does, and
a push that leaves the plugin unchanged commits nothing.

To try local changes, build and add the output as a local marketplace:
`claude plugin marketplace add dist/plugins/claude-code` or
`codex plugin marketplace add dist/plugins/codex`. Codex refuses while a `distill` marketplace
from another source exists (`codex plugin marketplace remove distill` first); Claude Code
replaces the old source.

A new file under `plugins/distill` reaches both agents unless it sits in an agent directory.
`plugin.rs::outputs_hold_only_their_agents_files` checks the outputs against the sources.

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
