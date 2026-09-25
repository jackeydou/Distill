# Distill docs

How Distill works today. Why it works this way is in
[the foundation spec](../spec/2026-09-13-distill-foundation/README.md).

## Pages

| Page | Covers |
|---|---|
| [development.md](development.md) | Toolchain, repo layout, `mise run check`, testing with a throwaway setup |
| [vault-format.md](vault-format.md) | Vault layout and every file format in it; how the index is derived |
| [plugin.md](plugin.md) | The Codex / Claude Code plugin: hook, MCP tools, skill, launcher |
| [AGENTS.md](AGENTS.md) | Writing rules for every Markdown file |

## Packages

| Package | Contract |
|---|---|
| `crates/distill-core` | [README](../crates/distill-core/README.md): vault, index, save/recall/search/stats |
| `crates/distill-cli` | [README](../crates/distill-cli/README.md): the `distill` command |
| `plugins/distill` | [README](../plugins/distill/README.md): the agent plugin |

Touching more than one package: read both READMEs and [vault-format.md](vault-format.md).
