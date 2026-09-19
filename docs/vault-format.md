# Vault format

The vault is the user's data. Everything else, including the SQLite index, is derived from it
and can be rebuilt with `distill reindex`. The types behind these formats live in
`crates/distill-core/src/model.rs` and `crates/distill-core/src/vault.rs`.

Current format version: **1** (`schema: 1` in every file, `format = 1` in the manifest).

## Layout

```
<vault>/
  distill.toml
  notes/<yyyy>/<mm>/<note-id>-<slug>.md
  topics/<topic-id>.md
  tag-aliases/<alias-id>.yml
  annotations/<note-id>/<annotation-id>.md
```

Ids are ULIDs. The note path uses the note's creation month; the slug is the title in
lowercase with non-alphanumeric runs replaced by `-`, cut at 48 characters.

The index reads only these paths. It ignores any file or directory whose name starts with `.`
(sync-tool metadata such as `.stfolder`, and Distill's own `.distill-tmp-*` files) and anything
else in the vault.

## `distill.toml`

```toml
format = 1
vault_id = "01M3B0TMB9AVAHQBDA9CETY3GD"
created = "2026-09-13T22:48:00-07:00"
```

`vault_id` names the index file on each device (`<data dir>/index/<vault_id>.db`) and lets a
second device recognize a synced vault.

## Note

```markdown
---
schema: 1
id: 01M3B0TMKWD8MKJ2CNSWNE81S5
topic: 01M3B0TMJY1NVDA5ZY1J99G8AR
tags:
- sqlite
- macos
source:
  agent: codex
  session_id: 01a0d530-42ae-7731-8a1d-b4e07d9b837d
  cwd: /Users/me/project
  git_repo: github.com/me/project
created: 2026-09-13T22:48:00-07:00
---
# bun:sqlite 为什么加载不了扩展

## 问题

…

## 结论

…

## 要点

- …

## 仍不清楚

- …
```

| Field | Rule |
|---|---|
| `topic` | The topic this note belongs to. Resolved through `merged_into` when read |
| `tags` | 1 to 5 normalized tags (see below) |
| `source.agent` | `codex` or `claude-code` |
| `source.session_id` | UUID of the agent session; used to reopen it |
| `source.cwd` | Working directory of the session |
| `source.git_repo` | Optional |
| `created` | RFC 3339 with the device's offset |
| `updated` | Optional, RFC 3339 |

The body always starts with `# <title>`. The four `##` sections are recognized by their exact
headings; `要点` and `仍不清楚` are bullet lists and are omitted when empty. Other sections
are kept and searched but have no field.

## Topic

```markdown
---
schema: 1
id: 01M3B0TMJY1NVDA5ZY1J99G8AR
label: bun:sqlite 为什么加载不了扩展
created: 2026-09-13T22:48:00-07:00
---
```

A topic is one question. Merging topic A into B writes `merged_into: <B>` into A's file; notes
keep pointing at A and the index follows the chain. The body is an optional description.

## Tag alias

```yaml
schema: 1
from: sqlite3
to: sqlite
created: 2026-09-13T22:48:00-07:00
```

Renames or merges a tag without editing notes. Chains resolve to the final tag; a cycle stops
at the tag where it was detected.

Tags are normalized before they are stored: trimmed, lowercased, full-width characters folded
to ASCII, and runs of spaces, `_` and `-` collapsed into one `-`.

## Annotation

```markdown
---
schema: 1
id: 01M3B1…
note: 01M3B0TMKWD8MKJ2CNSWNE81S5
created: 2026-09-13T22:50:00-07:00
---
Your own understanding, in any Markdown.
```

`anchor` is reserved for annotations on a passage and is not written yet.

## Writing rules

- Every write goes through a temp file in the same directory and a rename.
- A device only creates new files or rewrites files it owns (a topic it merges, an annotation
  it edits). Notes are not rewritten after they are saved.
- Prose fields and annotations pass through secret redaction before they are written.
  Matches become `[REDACTED:<kind>]`.

## Conflicts and unreadable files

When a sync tool leaves two files with the same frontmatter `id` (for example
`… (conflicted copy 2026-09-13).md`), both are indexed, the shortest path is treated as the
current one, and `distill doctor` lists the pair. Nothing is merged automatically.

A file that fails to parse is skipped and listed by `distill doctor` with the reason. The rest
of the vault stays usable.
