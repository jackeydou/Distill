---
name: distill
description: Distill keeps notes of what the user learned in agent sessions and notices when they ask the same question again. Use it when the user asks to distill, save or remember what was worked out ("distill this", "记下来", "存一下"), asks whether they asked something before, asks about their past questions, and before answering a conceptual question in a session where the Distill hook is active.
---

# Distill

Read this once per session. It says when to call the Distill tools and how to fill them in.
Talk to the user in their language. Keep every Distill message to one or two lines; Distill
supports the conversation and never takes it over.

## What the hook gives you

Each user prompt carries lines injected by the Distill hook:

- `distill-source: <agent> <session_id> <cwd>` — the session you are in. `cwd` is everything
  after the session id and may contain spaces. Copy these values into `source` when saving.
  Never invent or edit them.
- `distill-suggest: on` or `off` — whether you may offer to distill (section 2).
- A line saying Distill is not set up: tell the user to run `distill init` if they ask for
  Distill, and otherwise ignore Distill.

No `distill-source` line means the hook is not running: you cannot save. Tell the user to run
`distill doctor`.

## 1. Before answering: recall

Call `distill_recall` with the user's question, in their words, before answering when the
question is conceptual: how or why something works, a design choice or trade-off, the root
cause of a problem, what a concept means. Do not recall for code edits, running commands,
lookups inside the current task, or follow-ups on a question you already recalled.

The results are candidates; judge relevance yourself. When a topic is clearly the same
question:

- Build on the earlier conclusion and on the user's annotations (their own understanding).
  Address what the earlier note listed as still unclear.
- Tell the user in one line, for example: "You asked this before (3 times); last note: <title>
  (<url>)." `url` opens the note in the Distill web UI; `reopen.link` reopens the old
  session; `reopen.command` does it from a terminal.

When nothing is relevant, say nothing about Distill.

## 2. After answering: offer to distill

Only when `distill-suggest: on`. At the end of an answer worth revisiting, ask once, in one
line, whether to distill it.

Worth it: a concept or mechanism explained, a decision with trade-offs, the root cause of a
bug, a topic the user followed up on several times. Not worth it: one-off command lookups,
routine code changes, small talk.

Offer at most once per question per session. If the user declines, do not offer again for it.

## 3. Distill

When the user agrees, or asks to distill:

1. Call `distill_recall` with the core question.
2. Pick the topic. If a returned topic is the same underlying question, even in different
   words, use its `topic_id`. A related but different question gets `"new"`.
3. Pick 1 to 5 tags. Use tags from the `tags` list recall returned. Put a tag in `new_tags`
   only when none fits. Tags name broad subjects (`sqlite`, `rust`, `react`), not the question.
   Follow the style of the existing tags.
4. Write the note in the user's language. See [references/note-format.md](references/note-format.md).
5. Call `distill_save` with `source` copied from the `distill-source` line.
6. Tell the user in one or two lines: saved, the title, the `url`, and when `ask_count` is
   above 1, that this is the Nth time they asked.

When `distill_save` returns an error:

| Error | Do |
|---|---|
| unknown tag | Use one of the suggested tags, or move the tag to `new_tags` |
| new tag is close to an existing one | Use the existing tag. Resend with `confirm_new: true` only if it really means something else |
| unknown topic | Call `distill_recall` again, or use `"new"` |
| invalid source | Do not retry with other values. Tell the user to run `distill doctor` |
| no Distill config | Tell the user to run `distill init` |

## 4. The user asks about their history

For questions like "what do I keep asking about?" or "which topics haven't I understood?":
call `distill_stats` for counts, then `distill_search` or `distill_recall` to read notes.
Topics asked two or more times without an annotation are the ones the user most likely has not
digested yet; say so. Users add annotations on a note's page (its `url`), or with
`distill annotate <note-id> <text>`.

If a result has `warnings`, mention them in one line; the links in it may not open until the
user runs `distill ui`.

## Terms

| Term | Meaning |
|---|---|
| note | One distilled Q&A, stored as a Markdown file |
| topic | One question. Notes asking the same thing share a topic; `ask_count` is its note count |
| tag | A broad subject label; a note has 1 to 5 |
| annotation | The user's own note on a note: their understanding |

Requires `distill` 0.0.1 or later. If every tool call fails, ask the user to run
`distill doctor`.
