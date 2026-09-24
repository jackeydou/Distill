# Distill plugin

The Codex and Claude Code plugin for Distill: a `UserPromptSubmit` hook, the `distill` MCP
server and the `distill` skill. It needs the `distill` binary (0.0.1 or later) on the same
machine, set up with `distill init`.

Install: [INSTALL.md](../../INSTALL.md). How it works: [docs/plugin.md](../../docs/plugin.md).

## Contract

- Plugin id `distill@distill` in both agents.
- MCP tools: `distill_recall`, `distill_save`, `distill_search`, `distill_stats`.
- The hook only injects context; it writes nothing and never blocks a prompt.
- All agent behavior lives in `skills/distill/SKILL.md`. Change behavior there, not in the
  hook or the binary.

## Limits

- `bin/distill-launch` is a POSIX shell script; Windows is not handled yet.
- In Claude Code the skill is invoked as `/distill:distill`; in Codex, ask for it by name.
