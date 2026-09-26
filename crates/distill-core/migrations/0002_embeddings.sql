-- Vectors for semantic recall (P4), in sqlite-vec `vec0` tables with cosine distance.
--
-- note_vec is keyed by `<model>:<hash of the embedded text>`, not by file: `distill
-- reindex` clears the file tables but keeps this one, so a rebuild does not re-run the
-- model. Keys no note uses any more are deleted when missing vectors are computed.
CREATE VIRTUAL TABLE note_vec USING vec0 (
    key    TEXT PRIMARY KEY,
    vector FLOAT[384] distance_metric=cosine
);

-- One vector per topic: the normalized mean of its notes' vectors. Rebuilt whenever the
-- set of (topic, note vector) pairs changes; `vec_state.topic_vec` holds the hash of that
-- set.
CREATE VIRTUAL TABLE topic_vec USING vec0 (
    topic_id TEXT PRIMARY KEY,
    vector   FLOAT[384] distance_metric=cosine
);

CREATE TABLE vec_state (
    name  TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Topic pairs the user marked "not the same question", so they are not suggested again.
-- Device-local: a dismissal is a hint, not vault data. topic_a < topic_b.
CREATE TABLE dismissed_pair (
    topic_a TEXT NOT NULL,
    topic_b TEXT NOT NULL,
    PRIMARY KEY (topic_a, topic_b)
);
