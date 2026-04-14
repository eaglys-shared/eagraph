INSERT INTO files (path, content_hash, last_indexed)
VALUES (:path, :content_hash, :last_indexed)
ON CONFLICT(path) DO UPDATE SET
    content_hash = excluded.content_hash,
    last_indexed = excluded.last_indexed;
