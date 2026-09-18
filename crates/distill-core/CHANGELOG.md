# Changelog

## [Unreleased]

### Added
- Vault format 1: notes, topics, tag aliases and annotations as Markdown/YAML files, with a
  `distill.toml` manifest. See `docs/vault-format.md`.
- Per-device SQLite index derived from the vault: incremental sync by mtime, size and content
  hash; unreadable files and duplicate-id conflicts are reported, not fatal.
- `Distill::save` with tag rules: existing tags only in `tags`, near-duplicate `new_tags`
  rejected unless `confirm_new`, 1 to 5 tags per note.
- Recall ranked on CJK bigrams and words with IDF weighting, so reworded questions match.
- Keyword search on a trigram FTS index, falling back to `LIKE` below three characters.
- Stats by topic, tag, ISO week and project.
- Secret redaction on every prose field before it is written.
- `init`, `annotate`, `move_vault`, `use_vault` and `doctor` operations.
