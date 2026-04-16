WITH RECURSIVE reachable(id, depth) AS (
    SELECT target, 1 FROM edges WHERE source = :symbol_id
    UNION
    SELECT e.target, r.depth + 1
    FROM edges e JOIN reachable r ON e.source = r.id
    WHERE r.depth < :max_depth
)
SELECT s.id, s.name, s.kind, s.file_path, s.line_start, s.line_end, s.metadata
FROM symbols s
JOIN reachable r ON s.id = r.id
GROUP BY s.id
ORDER BY MIN(r.depth);
