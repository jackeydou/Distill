# Changelog

## [Unreleased]

### Added
- When no `distill` is installed, `bin/distill-launch` downloads the release pinned in
  `bin/distill-version` from GitHub releases, checks its SHA-256, and keeps it under
  `<data dir>/bin/<version>/`. Only the MCP server downloads; the hook still exits 0 at once.
  `DISTILL_RELEASES_URL` overrides the source. See `docs/plugin.md`.
- First-time setup in the skill: when Distill is not set up, the agent asks the user where to
  keep the vault, never picking one itself, and runs `init --vault` with their answer.
- `scripts/build-plugins.sh` writes `bin/distill-version` from the workspace version.
- Release workflow: a `v<version>` tag publishes `distill` for macOS and Linux, arm64 and
  x86_64, as `distill-<target>.tar.gz` plus a `.sha256`.
- Plugin for Codex and Claude Code: `UserPromptSubmit` hook, `distill` MCP server, `distill`
  skill, and a launcher that finds the `distill` binary outside the app's PATH.
- Per-agent builds in each agent's native layout: `scripts/build-plugins.sh`
  (`mise run build:plugins`) writes `dist/plugins/claude-code` and `dist/plugins/codex`.
- The skill hands the user each note's web UI `url` instead of its file path, and points
  annotations at the note's page.
- Marketplace branches `marketplace-codex` and `marketplace-claude`, published by CI on every
  push to `main`. Published versions carry a build suffix such as `0.0.1+codex.<commit>`.

