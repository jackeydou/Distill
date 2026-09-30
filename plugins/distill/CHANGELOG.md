# Changelog

## [Unreleased]

### Added
- Codex branding paths now resolve to the packaged SVG icon and flask wordmark logo.
- Codex MCP App with global sidebar and thread entrypoints; Distill dev has its own entry title.
- `mise run build:plugins:dev` builds **Distill dev** for Codex in `dist/plugins-dev/codex`.
  Its `distill-dev` marketplace, MCP server and skill are separate from the regular plugin.
  The launcher uses the checkout's debug binary and keeps config, index and vault in `dist/dev/`.
- Round flask brand assets in `codex/assets/`: a teal and gold icon, a Distill wordmark,
  and a transparent black outline icon for the Codex sidebar.
- Transparent icon concepts in `codex/assets/`, including stacked bookmarks.
- When no `distill` is installed, `bin/distill-launch` downloads the release pinned in
  `bin/distill-version` from GitHub releases, checks its SHA-256, and keeps it under
  `<data dir>/bin/<version>/`. Only the MCP server downloads; the hook still exits 0 at once.
  `DISTILL_RELEASES_URL` overrides the source. See `docs/plugin.md`.
- First-time setup in the skill: when Distill is not set up, the agent asks the user where to
  keep the vault, never picking one itself, and runs `init --vault` with their answer.
- `scripts/build-plugins.sh` writes `bin/distill-version` from the workspace version.
- Release workflow: a `v<version>` tag publishes `distill` for Apple Silicon Macs and Linux
  (arm64, x86_64, glibc 2.39+) as `distill-<target>.tar.gz` plus a `.sha256`. A manual run
  builds without publishing. No Intel Mac build: ort-sys
  has no ONNX Runtime for it.
- Plugin for Codex and Claude Code: `UserPromptSubmit` hook, `distill` MCP server, `distill`
  skill, and a launcher that finds the `distill` binary outside the app's PATH.
- Per-agent builds in each agent's native layout: `scripts/build-plugins.sh`
  (`mise run build:plugins`) writes `dist/plugins/claude-code` and `dist/plugins/codex`.
- The skill hands the user each note's web UI `url` instead of its file path, and points
  annotations at the note's page.
- Marketplace branches `marketplace-codex` and `marketplace-claude`, published by CI on every
  push to `main`. Published versions carry a build suffix such as `0.0.1+codex.<commit>`.
