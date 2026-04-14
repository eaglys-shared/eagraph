SELECT path, content_hash, last_indexed
FROM files
WHERE path = :path;
