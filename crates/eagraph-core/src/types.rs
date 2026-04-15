use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Opaque symbol identifier. Generated as hash(file_path + name + kind).
/// Scoped within a single branch DB. Does not encode repo or branch.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolId(pub String);

impl fmt::Display for SymbolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Opaque repo identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepoId(pub String);

impl fmt::Display for RepoId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Opaque identifier for an unresolved cross-repo reference (maps to autoincrement PK).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UnresolvedCrossRefId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Class,
    Method,
    Module,
    Variable,
    Type,
}

impl fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Function => "function",
            Self::Class => "class",
            Self::Method => "method",
            Self::Module => "module",
            Self::Variable => "variable",
            Self::Type => "type",
        };
        f.write_str(s)
    }
}

impl std::str::FromStr for SymbolKind {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "function" => Ok(Self::Function),
            "class" => Ok(Self::Class),
            "method" => Ok(Self::Method),
            "module" => Ok(Self::Module),
            "variable" => Ok(Self::Variable),
            "type" => Ok(Self::Type),
            other => Err(format!("unknown SymbolKind: {:?}", other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeKind {
    Calls,
    Imports,
    Inherits,
    References,
    TypeOf,
}

impl fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Calls => "calls",
            Self::Imports => "imports",
            Self::Inherits => "inherits",
            Self::References => "references",
            Self::TypeOf => "typeof",
        };
        f.write_str(s)
    }
}

impl std::str::FromStr for EdgeKind {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "calls" => Ok(Self::Calls),
            "imports" => Ok(Self::Imports),
            "inherits" => Ok(Self::Inherits),
            "references" => Ok(Self::References),
            "typeof" => Ok(Self::TypeOf),
            other => Err(format!("unknown EdgeKind: {:?}", other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    /// What does this symbol depend on.
    Outgoing,
    /// What depends on this symbol.
    Incoming,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RetrievalMethod {
    Structural,
    Semantic,
    Combined,
}

/// A code symbol extracted from source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub file_path: PathBuf,
    pub line_start: u32,
    pub line_end: u32,
    pub metadata: Option<serde_json::Value>,
}

/// An intra-repo edge between two symbols.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub source: SymbolId,
    pub target: SymbolId,
    pub kind: EdgeKind,
}

/// An edge produced by the extractor before resolution.
/// The target is a name string, not a resolved SymbolId.
/// The indexer resolves these into `Edge` after all symbols are collected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawEdge {
    pub source: SymbolId,
    pub target_name: String,
    pub kind: EdgeKind,
}

/// Convert a `&Path` to `&str`, returning an error if the path contains non-UTF-8 bytes.
///
/// eagraph assumes all repo paths are UTF-8. This helper makes the assumption explicit
/// by bailing with a clear error instead of silently substituting an empty string.
pub fn path_to_str(p: &std::path::Path) -> crate::Result<&str> {
    p.to_str().ok_or_else(|| {
        crate::EagraphError::Other(format!("path is not valid UTF-8: {}", p.display()))
    })
}

impl RawEdge {
    /// Resolve raw edges into real edges.
    /// `ext_to_lang` maps file extensions to language names (e.g. "py" → "python", "ts" → "typescript").
    /// Edges resolve only within the same language. Unresolvable edges are dropped.
    pub fn resolve(
        raw_edges: &[RawEdge],
        symbols: &[Symbol],
        ext_to_lang: &std::collections::HashMap<String, String>,
    ) -> Vec<Edge> {
        use std::collections::HashMap;

        // Single pass: build both id→lang and (name, lang)→id lookups
        let mut id_to_lang: HashMap<&str, &str> = HashMap::with_capacity(symbols.len());
        let mut name_lang_to_id: HashMap<(&str, &str), &SymbolId> =
            HashMap::with_capacity(symbols.len());
        for s in symbols {
            let Some(ext) = s.file_path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            let Some(lang) = ext_to_lang.get(ext) else {
                continue;
            };
            id_to_lang.insert(s.id.0.as_str(), lang.as_str());
            name_lang_to_id.insert((s.name.as_str(), lang.as_str()), &s.id);
        }

        raw_edges
            .iter()
            .filter_map(|re| {
                let src_lang = id_to_lang.get(re.source.0.as_str())?;
                let tid = name_lang_to_id.get(&(re.target_name.as_str(), *src_lang))?;
                Some(Edge {
                    source: re.source.clone(),
                    target: (*tid).clone(),
                    kind: re.kind,
                })
            })
            .collect()
    }
}

/// A resolved cross-repo edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossRepoEdge {
    pub source_repo: String,
    pub source_branch: String,
    pub source_symbol: SymbolId,
    pub target_repo: String,
    pub target_branch: String,
    pub target_symbol: SymbolId,
    pub kind: EdgeKind,
}

/// An import that references a symbol in another repo but hasn't been resolved yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnresolvedCrossRef {
    pub id: Option<UnresolvedCrossRefId>,
    pub source_repo: String,
    pub source_branch: String,
    pub source_symbol: SymbolId,
    pub target_package: String,
    pub target_path: String,
    pub kind: EdgeKind,
    pub created_at: u64,
}

/// Metadata about a registered repo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoRecord {
    pub id: RepoId,
    pub name: String,
    pub root: PathBuf,
}

/// Tracks per-file indexing state for change detection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRecord {
    pub path: PathBuf,
    pub content_hash: String,
    pub last_indexed: u64,
}

/// Generic metadata attached to a symbol by an enricher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Annotation {
    pub symbol_id: SymbolId,
    pub source: String,
    pub key: String,
    pub value: String,
}

/// A subgraph returned from intra-repo graph traversal.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubGraph {
    pub symbols: Vec<Symbol>,
    pub edges: Vec<Edge>,
}

/// A symbol paired with its source snippet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolWithSnippet {
    pub symbol: Symbol,
    pub snippet: String,
    pub annotations: Vec<Annotation>,
}

/// Result returned by the context retriever to the MCP layer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextResult {
    pub symbols: Vec<SymbolWithSnippet>,
    pub edges: Vec<Edge>,
    pub cross_repo_edges: Vec<CrossRepoEdge>,
    pub unresolved_refs: Vec<UnresolvedCrossRef>,
    pub repos_involved: Vec<RepoId>,
    pub retrieval_method: Option<RetrievalMethod>,
}
