DELETE FROM annotations WHERE symbol_id IN (SELECT id FROM symbols WHERE file_path = :file_path);
DELETE FROM edges WHERE source IN (SELECT id FROM symbols WHERE file_path = :file_path)
                     OR target IN (SELECT id FROM symbols WHERE file_path = :file_path);
DELETE FROM symbols WHERE file_path = :file_path;
DELETE FROM files WHERE path = :file_path;
