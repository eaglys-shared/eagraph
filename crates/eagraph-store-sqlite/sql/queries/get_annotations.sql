SELECT symbol_id, source, key, value
FROM annotations
WHERE symbol_id = :symbol_id;
