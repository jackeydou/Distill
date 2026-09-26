# Development

## Toolchain

`mise` pins the tools and runs the tasks ([mise.toml](../mise.toml)).

```bash
mise install
mise run check
```

`mise run check` runs `cargo fmt --check`, `cargo clippy -D warnings` and `cargo test` for the
whole workspace. A change is not done until it passes.

## Layout

| Path | What lives there |
|---|---|
| `crates/distill-core` | Library: vault files, SQLite index, operations shared by every front end |
| `crates/distill-cli` | The `distill` binary |
| `crates/distill-core/migrations` | Index schema as numbered SQL files |
| `plugins/distill` | Codex / Claude Code plugin ([plugin.md](plugin.md)) |
| `packaging/` | Marketplace manifests, one directory per agent |
| `scripts/` | Build scripts run by `mise` tasks |
| `.github/workflows/` | CI: `mise run check`, and publishing the plugin branches |
| `dist/` | Build output, not committed |
| `spec/` | One directory per decision ([spec/AGENTS.md](../spec/AGENTS.md)) |
| `.agents/skills/` | Skills for agents working in this repo |

The web UI (`apps/web`) arrives in a later phase.

To try the plugin from this checkout, run `mise run build:plugins` and add
`dist/plugins/claude-code` or `dist/plugins/codex` as a local marketplace, then install
`distill@distill`. Tests in `crates/distill-cli/tests/plugin.rs` check the built outputs.
Build and publish: [plugin.md](plugin.md#build-and-publish).

## Running against a throwaway setup

Two environment variables keep experiments away from your real vault:

| Variable | Effect |
|---|---|
| `DISTILL_HOME` | Puts config under `$DISTILL_HOME/config` and indexes under `$DISTILL_HOME/data` |
| `DISTILL_VAULT` | Uses this vault instead of the one in the config |

```bash
export DISTILL_HOME="$(mktemp -d)"
cargo run -p distill-cli -- init --vault "$DISTILL_HOME/vault"
cargo run -p distill-cli -- stats
```

Tests build their own setups the same way; see `crates/distill-cli/tests/cli.rs`.

## Changing the index schema

Add a numbered SQL file under `crates/distill-core/migrations/` and register it in
`crates/distill-core/src/index/mod.rs`. Never edit a migration that has shipped. The index is
rebuilt from the vault by `distill reindex`, so a migration may drop and recreate tables.

## Changing a vault file format

Read [vault-format.md](vault-format.md) first. A format change bumps `schema`, must keep
reading older files, and needs the user's decision on compatibility before you write it.
