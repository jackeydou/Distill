# distill

The `distill` command. Every command accepts `--json` for machine-readable output; errors go
to stderr with a non-zero exit code.

## Install

```bash
cargo install --path crates/distill-cli
distill init
```

`distill init` proposes `iCloud Drive/Distill` on macOS when iCloud Drive is on, otherwise
`Distill` in your documents folder. Pass `--vault <dir>` to choose; it accepts `~/…` and, on
macOS, `icloud:<path>`. Point a second device at the synced directory and `init` joins the
existing vault.

## Commands

| Command | Does |
|---|---|
| `init [--vault <dir>] [--adopt] [--yes]` | Create or join a vault; record device id, UI port and this binary's path |
| `save [--input <file>] [--topic <id\|new>]` | Save a note from a JSON `SaveRequest` on stdin or in a file |
| `recall <question…> [--limit 5]` | Topics you asked about before that resemble the question |
| `search [<query…>] [--tag <tag>] [--limit 20]` | Keyword search; no query lists recent notes |
| `stats` | Notes, topics, repeats, tags, weeks, projects |
| `tags` | Tags in use with note counts |
| `annotate <note-id> <text…>` | Add your own understanding to a note |
| `reindex` | Rebuild this device's index from the vault |
| `vault show \| use <dir> \| move <dir>` | Show, switch, or copy-and-switch the vault |
| `config list \| get <key> \| set <key> <value>` | Settable keys: `ui.port`, `suggest.enabled` |
| `doctor` | Paths, conflicts, unreadable files; exits non-zero when something needs attention |

The `SaveRequest` fields and tag rules are in the
[distill-core README](../distill-core/README.md#save-contract).

## Environment

| Variable | Effect |
|---|---|
| `DISTILL_HOME` | Use `$DISTILL_HOME/config` and `$DISTILL_HOME/data` instead of the platform directories |
| `DISTILL_VAULT` | Use this vault instead of the configured one |
