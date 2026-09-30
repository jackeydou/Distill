# Bug fixes

## 2026-09-30 — Codex build has no recognized branding or sidebar entrypoint

**Symptom.** The installed Distill dev plugin shows no branded icon or logo and has no
MCP sidebar UI to pin.
**Root cause.** Brand images were packaged without manifest references. The Codex server
exposed only four agent tools, with no UI resource or `global` entrypoint metadata.
**Fix.** The manifest points to packaged assets. Codex launches `mcp --ui`, which advertises
an embedded MCP App and global/thread entrypoints with SVG icons. The App reuses the UI API.
**Guard.** `tests/plugin.rs::codex_brand_paths_resolve_and_only_codex_enables_the_app`,
`tests/mcp.rs::ui_entrypoints_advertise_a_self_contained_app_and_icon`,
`ui_bridge_edits_the_same_vault_and_excludes_server_control_routes`;
`apps/web/src/api/mcp.test.ts` covers transport errors and subscription cleanup.
**Touches.** The 2026-09-29 launcher fix: plain MCP and Claude Code retain four tools.
The dev launcher still pins local binary and data paths; it additionally supplies the dev UI title.
Pin preferences belong to the host and are not written by the plugin.

## 2026-09-29 — Plugin from the marketplace cannot start: binary not found

**Symptom.** After installing `distill@distill` from `marketplace-claude` or
`marketplace-codex`, the MCP server exited 127 with "distill: binary not found", and the hook
injected nothing. The marketplace branches hold no binary, and no prebuilt binary existed
anywhere: the only way to get one was building from source.
**Root cause.** The plugin build excluded `distill` on the assumption that it "ships
separately", but nothing shipped it. `bin/distill-launch` could only search for an existing
install.
**Fix.** A release workflow publishes per-target binaries on `v*` tags
(`.github/workflows/release.yml`). `scripts/build-plugins.sh` pins the workspace version in
`bin/distill-version`, and `bin/distill-launch` downloads and verifies that release when no
`distill` is installed. The hook's not-set-up line now names the downloaded binary's path,
since `distill` is not on PATH (`crates/distill-cli/src/hook.rs`).
**Guard.** `crates/distill-cli/tests/plugin.rs::launcher_downloads_the_pinned_release_once`,
`launcher_hook_never_downloads`, `launcher_refuses_a_bad_checksum`,
`launcher_reports_a_missing_release`,
`launcher_prefers_the_pinned_release_over_a_recorded_download`,
`launcher_pins_the_workspace_version`; `tests/mcp.rs::hook_names_an_init_command_that_runs`.
**Touches.** The launcher's lookup order, which also serves locally built installs: the
download comes last, so `$DISTILL_BIN`, the recorded `bin_path`, PATH and the common install
directories still win. A recorded `bin_path` inside `<data dir>/bin/` is skipped so an old
download never shadows a newer pinned release. Keep stdout clean in the launcher; it is the MCP
transport. The published plugin pins a release tag; bumping the workspace version without
pushing the matching tag breaks fresh installs.
