INSERT INTO annotations (symbol_id, source, key, value)
VALUES (:symbol_id, :source, :key, :value)
ON CONFLICT(symbol_id, source, key) DO UPDATE SET
    value = excluded.value;
