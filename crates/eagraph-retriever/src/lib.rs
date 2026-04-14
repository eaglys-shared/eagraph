mod snippet;

use std::path::Path;

use eagraph_core::*;

pub use snippet::read_snippet;

/// Result of a context retrieval — symbols with source snippets and their edges.
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
pub fn get_context(
    store: &dyn GraphStore,
    repo_root: &Path,
    symbol_name: &str,
    depth: u32,
    context_lines: u32,
) -> Result<Option<ContextResult>> {
    let symbols = store.search_symbols(symbol_name, None)?;
    let sym = match symbols.into_iter().find(|s| s.name == symbol_name) {
        Some(s) => s,
        None => return Ok(None),
    };

    let subgraph = store.get_neighbors(&sym.id, Direction::Both, depth)?;

    let root_snippet = read_snippet(repo_root, &sym, context_lines);
    let root = ContextEntry {
        snippet: root_snippet,
        symbol: sym,
    };

    let neighbors: Vec<ContextEntry> = subgraph
        .symbols
        .into_iter()
        .map(|s| {
            let snippet = read_snippet(repo_root, &s, context_lines);
            ContextEntry { symbol: s, snippet }
        })
        .collect();

    Ok(Some(ContextResult {
        root,
        neighbors,
        edges: subgraph.edges,
    }))
}

/// Get dependents of a file: all symbols in the file + what depends on them (incoming edges).
pub fn get_dependents(
    store: &dyn GraphStore,
    repo_root: &Path,
    file_path: &Path,
    depth: u32,
    context_lines: u32,
) -> Result<Vec<ContextResult>> {
    let file_symbols = store.get_file_symbols(file_path)?;
    let mut results = Vec::new();

    for sym in file_symbols {
        // Skip module-level scope symbols
        if sym.kind == SymbolKind::Module {
            continue;
        }

        let subgraph = store.get_neighbors(&sym.id, Direction::Incoming, depth)?;
        if subgraph.symbols.is_empty() {
            continue;
        }

        let root_snippet = read_snippet(repo_root, &sym, context_lines);
        let root = ContextEntry {
            snippet: root_snippet,
            symbol: sym,
        };

        let neighbors: Vec<ContextEntry> = subgraph
            .symbols
            .into_iter()
            .map(|s| {
                let snippet = read_snippet(repo_root, &s, context_lines);
                ContextEntry { symbol: s, snippet }
            })
            .collect();

        results.push(ContextResult {
            root,
            neighbors,
            edges: subgraph.edges,
        });
    }

    Ok(results)
}
