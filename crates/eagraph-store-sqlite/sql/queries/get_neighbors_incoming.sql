WITH RECURSIVE reachable(id, depth) AS (
    SELECT source, 1 FROM edges WHERE target = :symbol_id
    UNION
    SELECT e.source, r.depth + 1
    FROM edges e JOIN reachable r ON e.target = r.id
    WHERE r.depth < :max_depth
)
SELECT DISTINCT s.id, s.name, s.kind, s.file_path, s.line_start, s.line_end, s.metadata
FROM symbols s
JOIN reachable r ON s.id = r.id;
