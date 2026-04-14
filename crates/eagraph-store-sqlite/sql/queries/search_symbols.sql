SELECT id, name, kind, file_path, line_start, line_end, metadata
FROM symbols
WHERE name LIKE '%' || :query || '%'
ORDER BY
    CASE WHEN name = :query THEN 0 ELSE 1 END,
    name
LIMIT 100;
