# Changelog

## [Unreleased]

### Added
- `distill` commands: `init`, `save`, `recall`, `search`, `stats`, `tags`, `annotate`,
  `reindex`, `vault show|use|move`, `config list|get|set`, `doctor`. All accept `--json`.
- `init` proposes iCloud Drive on macOS or the documents folder elsewhere, accepts `~/…` and
  `icloud:<path>`, and refuses to run without `--vault` or `--yes` when stdin is not a terminal.
