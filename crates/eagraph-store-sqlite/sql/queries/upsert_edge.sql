INSERT INTO edges (source, target, kind)
VALUES (:source, :target, :kind)
ON CONFLICT(source, target, kind) DO NOTHING;
