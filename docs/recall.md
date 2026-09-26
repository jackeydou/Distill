# Recall and duplicate topics

Recall answers "have I asked this before?". It is behind `distill recall`, the
`distill_recall` MCP tool and `GET /api/recall`, all through `Distill::recall`. It returns
candidate topics; the agent decides whether one is the same question.

## Two rankings

| Ranking | Always on | How | Code |
|---|---|---|---|
| Keyword | Yes | Every note scored on CJK bigrams and words, IDF-weighted, title and question counted double; notes covering less than 25% of the question's weight are dropped | `crates/distill-core/src/index/recall.rs` |
| Semantic | After `distill model pull` | A sqlite-vec KNN query (cosine distance) for the 30 notes whose `title + question` vectors are nearest the question's; notes below 0.40 similarity are dropped | `crates/distill-core/src/index/semantic.rs` |

With the model installed the two lists are merged by reciprocal rank fusion
(`1 / (60 + rank)` summed per note), so a note either ranking finds is a candidate and one
both find ranks first. Notes are then grouped into topics, best first. The result's
`semantic` field says whether embeddings took part.

Keyword search (`distill search`, the notes page) does not use embeddings.

## The model

`distill model pull` downloads
[`Qdrant/paraphrase-multilingual-MiniLM-L12-v2-onnx-Q`](https://huggingface.co/Qdrant/paraphrase-multilingual-MiniLM-L12-v2-onnx-Q)
(384 dimensions, about 0.25 GB on disk) into `<data dir>/models` and embeds every note
already in the vault. It runs in-process on ONNX Runtime, which is linked into `distill`
(fastembed with its prebuilt runtime). `HF_HOME` and `HF_ENDPOINT` change where the files
come from and are cached.

Nothing downloads implicitly. Without the model, recall is keyword-only, duplicate
suggestions are empty, and `distill doctor` says so without counting it as a problem.
`distill model status` reports whether it is installed.

A process loads the model once, on the first call that needs it (a few hundred ms), and
keeps it; the MCP server and web server reuse it across calls.

## Vectors

Vectors live in two sqlite-vec `vec0` tables in the index, both with cosine distance
(`crates/distill-core/migrations/0002_embeddings.sql`):

| Table | Key | Holds |
|---|---|---|
| `note_vec` | `<model>:<BLAKE3 of the embedded text>` | One vector per distinct note text |
| `topic_vec` | topic id (after merges) | The normalized mean of the topic's note vectors |

Notes that lack a vector are embedded on the next recall or duplicate check; keys no note uses
any more are deleted then. `topic_vec` is rewritten only when the set of (topic, note key)
pairs changes, which a hash in `vec_state` detects. `distill reindex` leaves these tables
alone, so a rebuild does not re-run the model.

sqlite-vec is compiled into `distill` (the `sqlite-vec` crate) and registered with SQLite as
an auto-extension before the index opens (`crates/distill-core/src/index/vec.rs`). That
registration is the workspace's only `unsafe` code; the lint is `deny`, and that module opts
out.

## Duplicate topics

Each topic's five nearest topics come from a KNN query on `topic_vec`. Pairs at 0.65
similarity or above are offered as probably the same question: on the web UI's Topics page
("可能重复"), as a banner on the home page, and as "相似的 topic" on a topic page (0.40 and
above, top 5).
Merging writes `merged_into` as described in [vault-format.md](vault-format.md#topic).
"不是同一个问题" records the pair in the index table `dismissed_pair`, on this device only.

## Thresholds

Both cut-offs were measured on 12 Chinese and English question pairs (2026-09-26):
rewordings of one question scored 0.40 to 0.74; different questions on the same subject
scored up to 0.54. The ranges overlap, which is why recall uses the low floor and leaves the
judgment to the agent, and merge suggestions use the high one and leave it to the user.
`recall_finds_reworded_questions_keywords_miss` in `crates/distill-core/tests/semantic.rs`
checks two rewordings keyword recall misses. It and the other model tests are `#[ignore]`d;
run them with a pulled model:

```bash
DISTILL_HOME=/tmp/distill-model distill model pull
DISTILL_TEST_MODEL_HOME=/tmp/distill-model cargo test -p distill-core --test semantic -- --include-ignored
```
