mod snippet;

use std::path::Path;

use eagraph_core::*;

pub use snippet::read_snippet;

/// Result of a context retrieval: symbols with source snippets and their edges.
pub struct ContextEntry {
    pub symbol: Symbol,
    pub snippet: String,
}

pub struct ContextResult {
    pub root: ContextEntry,
    pub neighbors: Vec<ContextEntry>,
    pub edges: Vec<Edge>,
}

/// Get structural context for a symbol: the symbol itself + its neighborhood + source snippets.
///
/// `limit` caps the number of returned neighbors. When the graph at the given depth
/// contains more symbols than the limit, neighbors are truncated and edges are filtered
/// to only those connecting retained symbols. Snippets are read only for retained
/// neighbors, so the limit also bounds I/O. Pass `None` for no cap.
pub fn get_context(
    store: &dyn GraphStore,
    repo_root: &Path,
    symbol_name: &str,
    depth: u32,
    context_lines: u32,
    limit: Option<usize>,
) -> Result<Option<ContextResult>> {
    let symbols = store.search_symbols(symbol_name, None)?;
    let sym = match symbols.into_iter().find(|s| s.name == symbol_name) {
        Some(s) => s,
        None => return Ok(None),
    };

    let subgraph = store.get_neighbors(&sym.id, Direction::Both, depth)?;

    let mut neighbor_syms = subgraph.symbols;
    let mut edges = subgraph.edges;

    if let Some(max) = limit {
        if neighbor_syms.len() > max {
            neighbor_syms.truncate(max);
            let retained: std::collections::HashSet<&SymbolId> =
                neighbor_syms.iter().map(|s| &s.id).collect();
            edges.retain(|e| {
                (e.source == sym.id || retained.contains(&e.source))
                    && (e.target == sym.id || retained.contains(&e.target))
            });
        }
    }

    let root_snippet = read_snippet(repo_root, &sym, context_lines);
    let root = ContextEntry {
        snippet: root_snippet,
        symbol: sym,
    };

    let neighbors: Vec<ContextEntry> = neighbor_syms
        .into_iter()
        .map(|s| {
            let snippet = read_snippet(repo_root, &s, context_lines);
            ContextEntry { symbol: s, snippet }
        })
        .collect();

    Ok(Some(ContextResult {
        root,
        neighbors,
        edges,
    }))
}

/// Get dependents of a file: all symbols in the file + what depends on them (incoming edges).
///
/// `limit` caps the number of neighbors per symbol (not the total across all symbols).
pub fn get_dependents(
    store: &dyn GraphStore,
    repo_root: &Path,
    file_path: &Path,
    depth: u32,
    context_lines: u32,
    limit: Option<usize>,
) -> Result<Vec<ContextResult>> {
    let file_symbols = store.get_file_symbols(file_path)?;
    let mut results = Vec::new();

    for sym in file_symbols {
        if sym.kind == SymbolKind::Module {
            continue;
        }

        let subgraph = store.get_neighbors(&sym.id, Direction::Incoming, depth)?;
        if subgraph.symbols.is_empty() {
            continue;
        }

        let mut neighbor_syms = subgraph.symbols;
        let mut edges = subgraph.edges;

        if let Some(max) = limit {
            if neighbor_syms.len() > max {
                neighbor_syms.truncate(max);
                let retained: std::collections::HashSet<&SymbolId> =
                    neighbor_syms.iter().map(|s| &s.id).collect();
                edges.retain(|e| {
                    (e.source == sym.id || retained.contains(&e.source))
                        && (e.target == sym.id || retained.contains(&e.target))
                });
            }
        }

        let root_snippet = read_snippet(repo_root, &sym, context_lines);
        let root = ContextEntry {
            snippet: root_snippet,
            symbol: sym,
        };

        let neighbors: Vec<ContextEntry> = neighbor_syms
            .into_iter()
            .map(|s| {
                let snippet = read_snippet(repo_root, &s, context_lines);
                ContextEntry { symbol: s, snippet }
            })
            .collect();

        results.push(ContextResult {
            root,
            neighbors,
            edges,
        });
    }

    Ok(results)
}
