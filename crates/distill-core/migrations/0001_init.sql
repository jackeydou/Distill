-- The index is a projection of the vault and can be rebuilt with `distill reindex`.
-- Rows are keyed by vault-relative path: two files with the same frontmatter id are a
-- sync conflict, and the *_current views pick the canonical file (shortest path).

CREATE TABLE file (
    path     TEXT PRIMARY KEY,
    kind     TEXT NOT NULL CHECK (kind IN ('note', 'topic', 'alias', 'annotation')),
    mtime_ns INTEGER NOT NULL,
    size     INTEGER NOT NULL,
    hash     TEXT NOT NULL,
    error    TEXT
);

CREATE TABLE note (
    path        TEXT PRIMARY KEY REFERENCES file (path) ON DELETE CASCADE,
    id          TEXT NOT NULL,
    topic_id    TEXT NOT NULL,
    title       TEXT NOT NULL,
    question    TEXT NOT NULL,
    conclusion  TEXT NOT NULL,
    body        TEXT NOT NULL,
    agent       TEXT NOT NULL,
    session_id  TEXT NOT NULL,
    cwd         TEXT NOT NULL,
    git_repo    TEXT,
    created     TEXT NOT NULL,
    created_utc TEXT NOT NULL
);
CREATE INDEX note_by_id ON note (id);
CREATE INDEX note_by_topic ON note (topic_id);

CREATE TABLE note_tag (
    note_path TEXT NOT NULL REFERENCES note (path) ON DELETE CASCADE,
    tag       TEXT NOT NULL,
    PRIMARY KEY (note_path, tag)
);

CREATE TABLE topic (
    path        TEXT PRIMARY KEY REFERENCES file (path) ON DELETE CASCADE,
    id          TEXT NOT NULL,
    label       TEXT NOT NULL,
    merged_into TEXT,
    created     TEXT NOT NULL
);
CREATE INDEX topic_by_id ON topic (id);

CREATE TABLE tag_alias (
    path     TEXT PRIMARY KEY REFERENCES file (path) ON DELETE CASCADE,
    from_tag TEXT NOT NULL,
    to_tag   TEXT NOT NULL,
    created  TEXT NOT NULL
);

CREATE TABLE annotation (
    path    TEXT PRIMARY KEY REFERENCES file (path) ON DELETE CASCADE,
    id      TEXT NOT NULL,
    note_id TEXT NOT NULL,
    body    TEXT NOT NULL,
    created TEXT NOT NULL,
    updated TEXT
);
CREATE INDEX annotation_by_note ON annotation (note_id);

CREATE VIRTUAL TABLE note_fts USING fts5 (
    path UNINDEXED,
    title,
    question,
    body,
    tokenize = 'trigram'
);

CREATE VIEW note_current AS
SELECT * FROM (
    SELECT *, row_number() OVER (PARTITION BY id ORDER BY length(path), path) AS rn FROM note
) WHERE rn = 1;

CREATE VIEW topic_current AS
SELECT * FROM (
    SELECT *, row_number() OVER (PARTITION BY id ORDER BY length(path), path) AS rn FROM topic
) WHERE rn = 1;

CREATE VIEW annotation_current AS
SELECT * FROM (
    SELECT *, row_number() OVER (PARTITION BY id ORDER BY length(path), path) AS rn FROM annotation
) WHERE rn = 1;
