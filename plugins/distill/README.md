# Distill plugin

The Codex and Claude Code plugin for Distill: a `UserPromptSubmit` hook, the `distill` MCP
server and the `distill` skill. It needs the `distill` binary, set up with `distill init`. When
none is installed, the MCP server's launcher downloads the release the build pins in
`bin/distill-version` from GitHub releases.

Install: [INSTALL.md](../../INSTALL.md). How it works: [docs/plugin.md](../../docs/plugin.md).

## Contract

- Plugin id `distill@distill` in both agents, installed from the `marketplace-codex` or
  `marketplace-claude` branch. This directory is the build source and does not install as is.
- MCP tools: `distill_recall`, `distill_save`, `distill_search`, `distill_stats`.
- The hook only injects context; it writes nothing and never blocks a prompt.
- All agent behavior lives in `skills/distill/SKILL.md`. Change behavior there, not in the
  hook or the binary.

## Limits

- `bin/distill-launch` is a POSIX shell script; Windows is not handled yet.
- Downloads cover Apple Silicon Macs and Linux with glibc 2.39 or later (arm64, x86_64), and need
  `curl`.
  Intel Macs are not supported at all; elsewhere, install `distill` yourself.
- In Claude Code the skill is invoked as `/distill:distill`; in Codex, ask for it by name.
