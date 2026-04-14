CREATE TABLE IF NOT EXISTS symbols (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL,
    file_path   TEXT NOT NULL,
    line_start  INTEGER,
    line_end    INTEGER,
    metadata    TEXT
);

CREATE TABLE IF NOT EXISTS edges (
    source      TEXT NOT NULL REFERENCES symbols(id),
    target      TEXT NOT NULL REFERENCES symbols(id),
    kind        TEXT NOT NULL,
    PRIMARY KEY (source, target, kind)
);

CREATE TABLE IF NOT EXISTS files (
    path         TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    last_indexed INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS annotations (
    symbol_id   TEXT NOT NULL REFERENCES symbols(id),
    source      TEXT NOT NULL,
    key         TEXT NOT NULL,
    value       TEXT NOT NULL,
    PRIMARY KEY (symbol_id, source, key)
);

CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source);
CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target);
CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_path);
CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
CREATE INDEX IF NOT EXISTS idx_annotations_symbol ON annotations(symbol_id);
