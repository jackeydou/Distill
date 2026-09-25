# distill-core

The library behind every Distill front end: the vault (Markdown files, the source of truth),
the per-device SQLite index derived from it, and the operations that read and write both.

## Public API

| Item | Purpose |
|---|---|
| `config::Dirs` | Config and data directories. `DISTILL_HOME` overrides both |
| `config::LocalConfig` | Per-device settings: vault path, device id, UI port, `suggest.enabled` |
| `ops::init` | Create or join a vault and write this device's config. Idempotent |
| `Distill::open` | Load config, open the vault, open and sync the index |
| `Distill::save` | Validate a `SaveRequest`, write topic and note files, return `SaveResult` |
| `Distill::annotate` | Add an annotation to a note |
| `Distill::move_vault` / `use_vault` | Copy the vault elsewhere, or switch to another vault |
| `Distill::doctor` | Paths, installed plugin, conflicts, unreadable files |
| `Index::recall` | Topics that resemble a question, with all their notes, annotations and every tag in use |
| `Index::search` | Keyword search, optionally filtered by tag |
| `Index::stats` / `tags` | Counts by topic, tag, ISO week and project |
| `redact::redact` | Secret redaction applied before anything reaches the vault |
| `sources` | Hook input and agent detection, injected context, source verification, reopen links, installed-plugin detection |

File formats: [docs/vault-format.md](../../docs/vault-format.md).

## Save contract

`SaveRequest` is JSON with `title`, `question`, `conclusion`, optional `key_points` and
`open_questions`, `topic` (an existing topic id or `"new"`), `tags`, `new_tags`, optional
`confirm_new`, and `source` (`agent`, `session_id`, `cwd`, optional `git_repo`). Unknown fields
are rejected.

- `tags` may only name tags already in use (or alias targets). Anything else fails with
  `Error::UnknownTag` and suggestions.
- `new_tags` are normalized; one that matches an existing tag is reused. One that is close to
  an existing tag (edit similarity ≥ 0.8, or equal once `-` is removed) fails with
  `Error::SimilarTag` unless `confirm_new` is true.
- A note has 1 to 5 tags.
- `source.session_id` must be a UUID.

## Limits

- Recall ranks every note in memory on CJK bigrams and words, weighted by IDF. It is meant for
  a personal vault of thousands of notes; embeddings replace it later.
- Search uses a trigram FTS index for pieces of three or more characters and `LIKE` for
  shorter ones.
- The index is safe for concurrent processes (WAL, 5 s busy timeout). The vault is safe for
  file-sync tools as long as writers follow the rules in the format doc.
