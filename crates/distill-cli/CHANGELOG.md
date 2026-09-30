# Changelog

## [Unreleased]

### Changed
- The hook's not-set-up line tells the agent to ask the user for a vault folder and run
  `init --vault` itself. The command it names runs: `distill` when `distill` on PATH is the
  running binary, otherwise the binary's quoted path, as for a copy the plugin downloaded.

### Added
- `distill mcp --ui`: embedded MCP App, global/thread entrypoints, monochrome icon and
  app-only read/write tools over the existing UI API. Plain `mcp` keeps four agent tools.
- `distill` commands: `init`, `save`, `recall`, `search`, `stats`, `tags`, `annotate`,
  `reindex`, `vault show|use|move`, `config list|get|set`, `doctor`. All accept `--json`.
- `init` proposes iCloud Drive on macOS or the documents folder elsewhere, accepts `~/…` and
  `icloud:<path>`, and refuses to run without `--vault` or `--yes` when stdin is not a terminal.
- `distill mcp`: stdio MCP server with `distill_recall`, `distill_save`, `distill_search` and
  `distill_stats`.
- `distill hook user-prompt-submit`: prints the context the plugin injects on every prompt.
- `distill ui`: local web UI server at `http://distill.localhost:<ui.port>` with a session
  cookie from a one-time link, loopback-only listening, Host and Origin checks, a JSON API and
  server-sent change events. `distill ui stop` stops it. See `docs/web-ui.md`.
- `distill model pull` downloads the embedding model and embeds existing notes;
  `distill model status` reports it. `distill recall`, `distill_recall` and the web UI use it
  when installed; `distill_recall` returns `semantic`. `doctor` shows whether it is installed.
- Web API: `GET /api/timeline`, `GET /api/topics/{id}/similar`, `GET /api/duplicates`,
  `POST /api/duplicates/dismiss`.
- `distill mcp` starts the web UI in the background before returning note links, unless
  `ui.autostart` is `false`. `distill_recall` and `distill_search` notes now carry `url`.
