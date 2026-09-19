# Writing rules

Every Markdown file in this repo follows these rules: docs, specs, READMEs, ledgers, skills.
They exist so a reader can trust and act on what they read.

## Language

- Repo rules, `docs/`, package READMEs and ledgers are in English.
- A spec is written in the language its discussion happened in.
- Terms stay in English everywhere, including inside Chinese text: note, topic, tag,
  annotation, vault, index, source, distill, recall. Definitions:
  [spec Terms](../spec/2026-09-13-distill-foundation/README.md#terms).

## Writing

- Lead with what the reader needs to act. Background comes after, if at all.
- One claim per sentence. Prefer concrete nouns and numbers to adjectives.
- Name things exactly: the file, the command, the flag, the error text. Link to the file
  instead of describing where it is.
- State behavior as fact in the present tense. If something is planned, say which phase.
- If you did not verify a claim, say so and say how to verify it.
- Keep a fact in one place and link to it (see [AGENTS.md](../AGENTS.md#one-home-per-fact)).
- Use tables for comparisons and reference, lists for steps, prose for reasoning.
- Wrap lines at 100 characters. Code blocks and tables may run longer.

## Slop checklist

Delete or rewrite anything on this list before you call a document done.

- [ ] Openers and closers that say nothing: "In this document…", "Overall…", "In summary…".
- [ ] Intensifiers and filler: "very", "really", "robust", "seamless", "powerful",
      "comprehensive", "leverage", "utilize".
- [ ] Hedges that are not uncertainty: "should generally", "may potentially".
- [ ] Restating the heading in the first sentence.
- [ ] Lists of three where one item carries the point.
- [ ] Rationale in `docs/` (it belongs in the spec that made the decision) and change
      history in `docs/` (it belongs in `CHANGELOG.md`).
- [ ] Instructions that no longer match the code. Run the command you document.
- [ ] Links that do not resolve.
