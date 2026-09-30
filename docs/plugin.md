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
and needs `jq`. The build also writes `bin/distill-version`: the `version` from the root
`Cargo.toml`, which names the release the launcher downloads. The `distill` binary itself is
not in the build.

On every push to `main`, [CI](../.github/workflows/ci.yml) runs `mise run check`, then builds
each agent and commits the output to its branch:

| Branch | Holds |
|---|---|
| `marketplace-codex` | `dist/plugins/codex` |
| `marketplace-claude` | `dist/plugins/claude-code` |

The build id is the first 12 characters of the last commit that touched `plugins/distill`,
`packaging`, the build script or the root `Cargo.toml`. The published version changes only when the plugin does, and
a push that leaves the plugin unchanged commits nothing.

To try local changes, build and add the output as a local marketplace:
`claude plugin marketplace add dist/plugins/claude-code` or
`codex plugin marketplace add dist/plugins/codex`. Codex refuses while a `distill` marketplace
from another source exists (`codex plugin marketplace remove distill` first); Claude Code
replaces the old source.

A new file under `plugins/distill` reaches both agents unless it sits in an agent directory.
`plugin.rs::outputs_hold_only_their_agents_files` checks the outputs against the sources.

## Binary releases

Pushing a tag `v<version>` runs [the release workflow](../.github/workflows/release.yml). It
fails unless `<version>` equals the `version` in the root `Cargo.toml`. It builds `distill`
with the web UI embedded and publishes a GitHub release with two assets per target:
`distill-<target>.tar.gz`, holding one file named `distill`, and
`distill-<target>.tar.gz.sha256`. The launcher downloads these names, so they are a contract.

| Target | Built on |
|---|---|
| `aarch64-apple-darwin` | `macos-15` |
| `x86_64-unknown-linux-gnu` | `ubuntu-22.04` (glibc 2.35) |
| `aarch64-unknown-linux-gnu` | `ubuntu-22.04-arm` |

A published plugin pins the release named in its `bin/distill-version`. Until a release with
that tag exists, launchers without an installed `distill` fail with the download error.

## Finding the binary

Desktop apps often start hooks and MCP servers with a PATH that lacks the install directory.
`bin/distill-launch` tries, in order:

1. `$DISTILL_BIN`.
2. The `bin_path` that `distill init` recorded in the config, unless it points into the
   download directory below.
3. `PATH`, then `~/.cargo/bin`, `/opt/homebrew/bin`, `/usr/local/bin` and `~/.local/bin`.
4. The downloaded release: `<data dir>/bin/<version>/distill`, where `<version>` is the
   content of `bin/distill-version`. The data dir is `~/Library/Application Support/Distill`
   on macOS, `$XDG_DATA_HOME/distill` (default `~/.local/share/distill`) elsewhere, and
   `$DISTILL_HOME/data` when that is set.

When none exists, the MCP server's launcher downloads the release for the current platform
with `curl`, checks it against the `.sha256` asset, and unpacks it into step 4's path.
There is no Intel Mac build: ort-sys, which links ONNX Runtime for embeddings, has no prebuilt
library for `x86_64-apple-darwin`, so `distill` does not build there at all.
`DISTILL_RELEASES_URL` replaces the base URL
`https://github.com/jackeydou/Distill/releases/download`. On failure it exits 127 with the
reason on stderr: an unsupported platform, no `curl`, a failed download, or a checksum
mismatch. The hook never downloads: with no binary it exits 0 silently, so a prompt never
waits on the network.

Older downloaded releases stay in `<data dir>/bin/`. Delete them by hand.

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
the `suggest.enabled` setting. Without a config, the hook injects one line telling the agent to
ask the user where to keep the vault and then run `distill init --vault "<their folder>"`; the
skill's "First-time setup" lists the choices to offer. The command in it is `distill` when that
name on `PATH` is the running binary, and the binary's quoted path otherwise, which is the case
for a downloaded release.

## The MCP server

`distill mcp` serves on stdio. Every call opens the config, vault and index afresh.

| Tool | Returns |
|---|---|
| `distill_recall` | Matching topics with `ask_count`, their notes (with `url`, `file`, annotations and `reopen`), every tag in use, `semantic` (whether embeddings took part, [recall.md](recall.md)), and `warnings` |
| `distill_save` | The save result (`note_id`, `topic_id`, `ask_count`, `tags`, `url`, `file`) plus `warnings` |
| `distill_search` | Notes matching keywords and/or a tag, with `url` and `file` |
| `distill_stats` | The same counts as `distill stats --json` |

A failed validation comes back as a tool error whose text says what to change. Before saving,
the server checks the source: a Codex session id must match a rollout file under
`$CODEX_HOME/sessions` or `archived_sessions` (default `~/.codex`); a Claude Code session id
that differs from the server's `CLAUDE_CODE_SESSION_ID` is kept and logged as a warning,
because the id changes after `/clear` while the server keeps running.

`url` is the note's page in the web UI. Before returning one from `distill_save` or a
non-empty `distill_recall`, the server makes sure the web UI is running and starts it in the
background if not ([web-ui.md](web-ui.md)). When that fails, the tool still succeeds and the
reason is in `warnings`. `ui.autostart = false` turns this off.

`reopen` holds `link` (`codex://threads/<id>` or `claude://resume?session=<id>`) and
`command` (`codex resume <id>`, or `cd <cwd> && claude --resume <id>`).

## `distill doctor` and the plugin

`distill doctor` reports the plugin as installed when `~/.claude/plugins/installed_plugins.json`
(or `$CLAUDE_CONFIG_DIR`) lists `distill@…`, or `~/.codex/config.toml` (or `$CODEX_HOME`) has an
enabled `plugins."distill@…"` table. It is a problem when neither agent has it.
