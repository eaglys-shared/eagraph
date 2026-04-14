INSERT INTO symbols (id, name, kind, file_path, line_start, line_end, metadata)
VALUES (:id, :name, :kind, :file_path, :line_start, :line_end, :metadata)
ON CONFLICT(id) DO UPDATE SET
    name = excluded.name,
    kind = excluded.kind,
    file_path = excluded.file_path,
    line_start = excluded.line_start,
    line_end = excluded.line_end,
    metadata = excluded.metadata;
