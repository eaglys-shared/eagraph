use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use rusqlite::{params, Connection, named_params};

use eagraph_core::*;

use crate::sql;

pub struct SqliteGraphStore {
    conn: Mutex<Connection>,
}

impl SqliteGraphStore {
    fn conn(&self) -> Result<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| EagraphError::Store(format!("mutex poisoned: {}", e)))
    }
}

impl SqliteGraphStore {
    pub fn begin_transaction(&self) -> Result<()> {
        self.conn()?.execute_batch("BEGIN IMMEDIATE").map_err(map_err)
    }

    pub fn commit_transaction(&self) -> Result<()> {
        self.conn()?.execute_batch("COMMIT").map_err(map_err)
    }

    pub fn rollback_transaction(&self) -> Result<()> {
        self.conn()?.execute_batch("ROLLBACK").map_err(map_err)
    }

    pub fn get_all_symbols(&self) -> Result<Vec<Symbol>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT id, name, kind, file_path, line_start, line_end, metadata FROM symbols")
            .map_err(map_err)?;
        let rows = stmt.query_map([], row_to_symbol).map_err(map_err)?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(map_err)
    }

    pub fn get_all_edges(&self) -> Result<Vec<Edge>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT source, target, kind FROM edges")
            .map_err(map_err)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Edge {
                    source: SymbolId(row.get(0)?),
                    target: SymbolId(row.get(1)?),
                    kind: EdgeKind::from_str(&row.get::<_, String>(2)?)
                        .unwrap_or(EdgeKind::References),
                })
            })
            .map_err(map_err)?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(map_err)
    }

    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| EagraphError::Store(e.to_string()))?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| EagraphError::Store(e.to_string()))?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| EagraphError::Store(e.to_string()))?;
        conn.execute_batch(sql::BRANCH_SCHEMA)
            .map_err(|e| EagraphError::Store(e.to_string()))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

fn row_to_symbol(row: &rusqlite::Row) -> rusqlite::Result<Symbol> {
    let metadata_str: Option<String> = row.get(6)?;
    let metadata = metadata_str.and_then(|s| serde_json::from_str(&s).ok());
    Ok(Symbol {
        id: SymbolId(row.get(0)?),
        name: row.get(1)?,
        kind: SymbolKind::from_str(&row.get::<_, String>(2)?).unwrap_or(SymbolKind::Variable),
        file_path: PathBuf::from(row.get::<_, String>(3)?),
        line_start: row.get(4)?,
        line_end: row.get(5)?,
        metadata,
    })
}

fn map_err(e: rusqlite::Error) -> EagraphError {
    EagraphError::Store(e.to_string())
}

impl GraphStore for SqliteGraphStore {
    fn upsert_symbols(&self, symbols: &[Symbol]) -> Result<()> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(sql::UPSERT_SYMBOL).map_err(map_err)?;
        for s in symbols {
            let metadata_str = s.metadata.as_ref().map(|m| m.to_string());
            stmt.execute(named_params! {
                ":id": &s.id.0,
                ":name": &s.name,
                ":kind": s.kind.to_string(),
                ":file_path": s.file_path.to_str().unwrap_or(""),
                ":line_start": s.line_start,
                ":line_end": s.line_end,
                ":metadata": metadata_str,
            })
            .map_err(map_err)?;
        }
        Ok(())
    }

    fn upsert_edges(&self, edges: &[Edge]) -> Result<()> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(sql::UPSERT_EDGE).map_err(map_err)?;
        for e in edges {
            stmt.execute(named_params! {
                ":source": &e.source.0,
                ":target": &e.target.0,
                ":kind": e.kind.to_string(),
            })
            .map_err(map_err)?;
        }
        Ok(())
    }

    fn delete_file_data(&self, file_path: &Path) -> Result<()> {
        let conn = self.conn()?;
        let fp = file_path.to_str().unwrap_or("");
        // delete_file_data.sql has multiple statements, execute them individually
        conn.execute(
            "DELETE FROM annotations WHERE symbol_id IN (SELECT id FROM symbols WHERE file_path = ?1)",
            params![fp],
        ).map_err(map_err)?;
        conn.execute(
            "DELETE FROM edges WHERE source IN (SELECT id FROM symbols WHERE file_path = ?1) OR target IN (SELECT id FROM symbols WHERE file_path = ?1)",
            params![fp],
        ).map_err(map_err)?;
        conn.execute("DELETE FROM symbols WHERE file_path = ?1", params![fp])
            .map_err(map_err)?;
        conn.execute("DELETE FROM files WHERE path = ?1", params![fp])
            .map_err(map_err)?;
        Ok(())
    }

    fn upsert_file_record(&self, record: &FileRecord) -> Result<()> {
        let conn = self.conn()?;
        conn.prepare_cached(sql::UPSERT_FILE_RECORD)
            .map_err(map_err)?
            .execute(named_params! {
                ":path": record.path.to_str().unwrap_or(""),
                ":content_hash": &record.content_hash,
                ":last_indexed": record.last_indexed,
            })
            .map_err(map_err)?;
        Ok(())
    }

    fn get_symbol(&self, id: &SymbolId) -> Result<Option<Symbol>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare_cached(
                "SELECT id, name, kind, file_path, line_start, line_end, metadata FROM symbols WHERE id = ?1",
            )
            .map_err(map_err)?;
        let mut rows = stmt.query(params![&id.0]).map_err(map_err)?;
        match rows.next().map_err(map_err)? {
            Some(row) => Ok(Some(row_to_symbol(row).map_err(map_err)?)),
            None => Ok(None),
        }
    }

    fn search_symbols(&self, query: &str, kind: Option<SymbolKind>) -> Result<Vec<Symbol>> {
        let conn = self.conn()?;
        match kind {
            Some(k) => {
                let mut stmt = conn
                    .prepare_cached(sql::SEARCH_SYMBOLS_WITH_KIND)
                    .map_err(map_err)?;
                let rows = stmt
                    .query_map(
                        named_params! { ":query": query, ":kind": k.to_string() },
                        row_to_symbol,
                    )
                    .map_err(map_err)?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(map_err)
            }
            None => {
                let mut stmt = conn
                    .prepare_cached(sql::SEARCH_SYMBOLS)
                    .map_err(map_err)?;
                let rows = stmt
                    .query_map(named_params! { ":query": query }, row_to_symbol)
                    .map_err(map_err)?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(map_err)
            }
        }
    }

    fn get_file_symbols(&self, file_path: &Path) -> Result<Vec<Symbol>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare_cached(
                "SELECT id, name, kind, file_path, line_start, line_end, metadata FROM symbols WHERE file_path = ?1",
            )
            .map_err(map_err)?;
        let rows = stmt
            .query_map(params![file_path.to_str().unwrap_or("")], row_to_symbol)
            .map_err(map_err)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_err)
    }

    fn get_file_record(&self, file_path: &Path) -> Result<Option<FileRecord>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare_cached(sql::GET_FILE_RECORD)
            .map_err(map_err)?;
        let fp = file_path.to_str().unwrap_or("");
        let mut rows = stmt.query(named_params! { ":path": fp }).map_err(map_err)?;
        match rows.next().map_err(map_err)? {
            Some(row) => Ok(Some(FileRecord {
                path: PathBuf::from(row.get::<_, String>(0).map_err(map_err)?),
                content_hash: row.get(1).map_err(map_err)?,
                last_indexed: row.get(2).map_err(map_err)?,
            })),
            None => Ok(None),
        }
    }

    fn get_neighbors(
        &self,
        id: &SymbolId,
        direction: Direction,
        depth: u32,
    ) -> Result<SubGraph> {
        let conn = self.conn()?;
        let query = match direction {
            Direction::Outgoing => sql::GET_NEIGHBORS_OUTGOING,
            Direction::Incoming => sql::GET_NEIGHBORS_INCOMING,
            Direction::Both => sql::GET_NEIGHBORS_BOTH,
        };
        let mut stmt = conn.prepare(query).map_err(map_err)?;
        let symbols: Vec<Symbol> = stmt
            .query_map(
                named_params! { ":symbol_id": &id.0, ":max_depth": depth },
                row_to_symbol,
            )
            .map_err(map_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_err)?;

        // Collect all symbol IDs in the subgraph (including the root)
        let mut symbol_ids: std::collections::HashSet<&str> =
            symbols.iter().map(|s| s.id.0.as_str()).collect();
        symbol_ids.insert(&id.0);

        // Fetch edges between subgraph symbols
        // Use the same positional params for both IN clauses
        let n = symbol_ids.len();
        let src_placeholders: Vec<String> = (1..=n).map(|i| format!("?{i}")).collect();
        let tgt_placeholders: Vec<String> = (n + 1..=2 * n).map(|i| format!("?{i}")).collect();
        let edge_sql = format!(
            "SELECT source, target, kind FROM edges WHERE source IN ({}) AND target IN ({})",
            src_placeholders.join(","),
            tgt_placeholders.join(","),
        );
        let mut edge_stmt = conn.prepare(&edge_sql).map_err(map_err)?;
        let id_vec: Vec<&str> = symbol_ids.iter().copied().collect();
        let mut param_values: Vec<&dyn rusqlite::types::ToSql> = Vec::new();
        for id_str in &id_vec {
            param_values.push(id_str);
        }
        for id_str in &id_vec {
            param_values.push(id_str);
        }
        let edges: Vec<Edge> = edge_stmt
            .query_map(param_values.as_slice(), |row| {
                Ok(Edge {
                    source: SymbolId(row.get(0)?),
                    target: SymbolId(row.get(1)?),
                    kind: EdgeKind::from_str(&row.get::<_, String>(2)?)
                        .unwrap_or(EdgeKind::References),
                })
            })
            .map_err(map_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_err)?;

        Ok(SubGraph { symbols, edges })
    }

    fn get_shortest_path(
        &self,
        from: &SymbolId,
        to: &SymbolId,
    ) -> Result<Option<Vec<SymbolId>>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(sql::GET_SHORTEST_PATH).map_err(map_err)?;
        let mut rows = stmt
            .query(named_params! { ":from_id": &from.0, ":to_id": &to.0 })
            .map_err(map_err)?;
        match rows.next().map_err(map_err)? {
            Some(row) => {
                let trail: String = row.get(0).map_err(map_err)?;
                let path: Vec<SymbolId> = trail.split(',').map(|s| SymbolId(s.to_string())).collect();
                Ok(Some(path))
            }
            None => Ok(None),
        }
    }

    fn upsert_annotations(&self, annotations: &[Annotation]) -> Result<()> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare_cached(sql::UPSERT_ANNOTATION)
            .map_err(map_err)?;
        for a in annotations {
            stmt.execute(named_params! {
                ":symbol_id": &a.symbol_id.0,
                ":source": &a.source,
                ":key": &a.key,
                ":value": &a.value,
            })
            .map_err(map_err)?;
        }
        Ok(())
    }

    fn delete_annotations(&self, symbol_id: &SymbolId, source: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM annotations WHERE symbol_id = ?1 AND source = ?2",
            params![&symbol_id.0, source],
        )
        .map_err(map_err)?;
        Ok(())
    }

    fn get_annotations(&self, symbol_id: &SymbolId) -> Result<Vec<Annotation>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare_cached(sql::GET_ANNOTATIONS)
            .map_err(map_err)?;
        let rows = stmt
            .query_map(named_params! { ":symbol_id": &symbol_id.0 }, |row| {
                Ok(Annotation {
                    symbol_id: SymbolId(row.get(0)?),
                    source: row.get(1)?,
                    key: row.get(2)?,
                    value: row.get(3)?,
                })
            })
            .map_err(map_err)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_err)
    }
}
