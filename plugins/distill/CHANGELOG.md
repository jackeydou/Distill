# Changelog

## [Unreleased]

### Added
- Plugin for Codex and Claude Code: `UserPromptSubmit` hook, `distill` MCP server, `distill`
  skill, and a launcher that finds the `distill` binary outside the app's PATH.
- Per-agent builds in each agent's native layout: `scripts/build-plugins.sh`
  (`mise run build:plugins`) writes `dist/plugins/claude-code` and `dist/plugins/codex`.
- The skill hands the user each note's web UI `url` instead of its file path, and points
  annotations at the note's page.
- Marketplace branches `marketplace-codex` and `marketplace-claude`, published by CI on every
  push to `main`. Published versions carry a build suffix such as `0.0.1+codex.<commit>`.

