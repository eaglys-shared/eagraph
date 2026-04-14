WITH RECURSIVE path(id, trail) AS (
    SELECT :from_id, :from_id
    UNION
    SELECT e.target, p.trail || ',' || e.target
    FROM edges e JOIN path p ON e.source = p.id
    WHERE p.trail NOT LIKE '%' || e.target || '%'
)
SELECT trail FROM path WHERE id = :to_id
ORDER BY length(trail)
LIMIT 1;
