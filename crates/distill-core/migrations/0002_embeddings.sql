-- Note vectors for semantic recall (P4). Keyed by model and by the hash of the embedded
-- text, not by file: `distill reindex` clears the file tables but keeps these, so a rebuild
-- does not re-run the model. Rows for texts no longer in the vault are pruned when missing
-- vectors are computed.
CREATE TABLE embedding (
    model     TEXT NOT NULL,
    text_hash TEXT NOT NULL,
    vector    BLOB NOT NULL,
    PRIMARY KEY (model, text_hash)
);

-- Topic pairs the user marked "not the same question", so they are not suggested again.
-- Device-local: a dismissal is a hint, not vault data. topic_a < topic_b.
CREATE TABLE dismissed_pair (
    topic_a TEXT NOT NULL,
    topic_b TEXT NOT NULL,
    PRIMARY KEY (topic_a, topic_b)
);
