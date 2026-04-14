use std::path::Path;

use crate::error::Result;
use crate::types::*;

/// Persistence for a single branch DB (symbols, edges, files, annotations).
pub trait GraphStore: Send + Sync {
    // --- Writes ---

    fn upsert_symbols(&self, symbols: &[Symbol]) -> Result<()>;
    fn upsert_edges(&self, edges: &[Edge]) -> Result<()>;
    fn delete_file_data(&self, file_path: &Path) -> Result<()>;
    fn upsert_file_record(&self, record: &FileRecord) -> Result<()>;

    // --- Reads ---

    fn get_symbol(&self, id: &SymbolId) -> Result<Option<Symbol>>;
    fn search_symbols(&self, query: &str, kind: Option<SymbolKind>) -> Result<Vec<Symbol>>;
    fn get_file_symbols(&self, file_path: &Path) -> Result<Vec<Symbol>>;
    fn get_file_record(&self, file_path: &Path) -> Result<Option<FileRecord>>;

    // --- Graph traversal (intra-repo, within this branch DB) ---

    fn get_neighbors(
        &self,
        id: &SymbolId,
        direction: Direction,
        depth: u32,
    ) -> Result<SubGraph>;
    fn get_shortest_path(
        &self,
        from: &SymbolId,
        to: &SymbolId,
    ) -> Result<Option<Vec<SymbolId>>>;

    // --- Annotations ---

    fn upsert_annotations(&self, annotations: &[Annotation]) -> Result<()>;
    fn delete_annotations(&self, symbol_id: &SymbolId, source: &str) -> Result<()>;
    fn get_annotations(&self, symbol_id: &SymbolId) -> Result<Vec<Annotation>>;
}

/// Persistence for cross-repo edges and unresolved references (crossref.db).
pub trait CrossRefStore: Send + Sync {
    // --- Resolved edges ---

    fn upsert_edges(&self, edges: &[CrossRepoEdge]) -> Result<()>;
    fn delete_edges_for(&self, repo: &str, branch: &str) -> Result<()>;
    fn get_edges_from(
        &self,
        repo: &str,
        branch: &str,
        symbol: &SymbolId,
    ) -> Result<Vec<CrossRepoEdge>>;
    fn get_edges_to(
        &self,
        repo: &str,
        branch: &str,
        symbol: &SymbolId,
    ) -> Result<Vec<CrossRepoEdge>>;
    fn get_edges_to_symbols(
        &self,
        repo: &str,
        branch: &str,
        symbols: &[SymbolId],
    ) -> Result<Vec<CrossRepoEdge>>;

    // --- Unresolved refs ---

    fn upsert_unresolved(&self, refs: &[UnresolvedCrossRef]) -> Result<()>;
    fn delete_unresolved_for(&self, repo: &str, branch: &str) -> Result<()>;
    fn get_unresolved_targeting(&self, package: &str) -> Result<Vec<UnresolvedCrossRef>>;
    fn get_unresolved_from(
        &self,
        repo: &str,
        branch: &str,
    ) -> Result<Vec<UnresolvedCrossRef>>;
    fn delete_unresolved(&self, ids: &[UnresolvedCrossRefId]) -> Result<()>;
}

/// Vector embedding storage for semantic search.
pub trait EmbeddingStore: Send + Sync {
    fn upsert_embeddings(&self, items: &[(SymbolId, Vec<f32>)]) -> Result<()>;
    fn delete_embeddings(&self, ids: &[SymbolId]) -> Result<()>;
    fn search_nearest(&self, vector: &[f32], k: usize) -> Result<Vec<(SymbolId, f32)>>;
}

/// Post-indexing hook that attaches metadata to symbols.
pub trait Enricher: Send + Sync {
    fn name(&self) -> &str;
    fn enrich_symbols(&self, symbols: &[Symbol]) -> Result<Vec<Annotation>>;
    fn enrich_file(
        &self,
        repo: &RepoRecord,
        file_path: &Path,
    ) -> Result<Vec<Annotation>>;
}
