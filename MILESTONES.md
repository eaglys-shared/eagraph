# eagraph — Implementation Milestones

Milestones follow the crate dependency graph: core → store → parser → indexing → retriever → MCP → watcher → cross-repo → extras. Each milestone builds on the previous and is independently testable.

---

## M1: Workspace + Core Types

Scaffold the Cargo workspace and define every domain type and trait in `eagraph-core`. Other crates exist as stubs.

**Scope:**
- Cargo workspace with all crate directories per DESIGN.md project structure
- `eagraph-core`: all domain types (`Symbol`, `Edge`, `CrossRepoEdge`, `UnresolvedCrossRef`, `RepoRecord`, `FileRecord`, `Annotation`, `SubGraph`, `ContextResult`, `SymbolWithSnippet`)
- All enums: `SymbolKind`, `EdgeKind`, `Direction`, `RetrievalMethod`
- All traits: `GraphStore`, `CrossRefStore`, `EmbeddingStore`, `Enricher`
- Config types for `config.toml` deserialization
- Error types (`thiserror`)

**Done when:**
- `cargo check --workspace` passes
- All types and traits from DESIGN.md are defined and public
- Other crates compile as empty stubs (`lib.rs` only)

---

## M2: SQLite Store

Implement `SqliteGraphStore` against the `GraphStore` trait. All SQL lives in `.sql` files, embedded via `include_str!`.

**Scope:**
- `eagraph-store-sqlite`: `SqliteGraphStore` implementing `GraphStore`
- `sql/branch_schema.sql` — tables: `symbols`, `edges`, `files`, `annotations` with all indexes
- Query `.sql` files: `upsert_symbol`, `upsert_edge`, `delete_file_data`, `search_symbols`, `get_neighbors` (recursive CTE), `get_shortest_path`, `upsert_annotation`, `get_annotations`
- WAL mode enabled at connection init
- Transaction support (`begin_transaction`, `commit`)

**Done when:**
- Unit tests pass for all `GraphStore` methods using in-memory SQLite:
  - Upsert and retrieve symbols by ID
  - Upsert and retrieve edges
  - `delete_file_data` removes symbols, edges, and file records for a path
  - `search_symbols` with name query and optional kind filter
  - `get_neighbors` with depth parameter (recursive CTE)
  - `get_shortest_path` between two connected symbols
  - `upsert_file_record` / `get_file_record`
  - Annotation CRUD
- All SQL lives in `.sql` files, none in Rust strings

---

## M3: Parser (Python)

Tree-sitter integration with the first language extractor. Parse Python files into `Symbol` and `Edge` vectors.

**Scope:**
- `eagraph-parser`: tree-sitter setup, grammar loading
- Python extractor: extract `Function`, `Class`, `Method`, `Module`, `Variable` symbols
- Edge extraction: `Calls`, `Imports`, `Inherits`, `References`
- `SymbolId` generation: `hash(file_path + name + kind)`
- Parser trait or function: `parse_file(path, source) -> (Vec<Symbol>, Vec<Edge>)`

**Done when:**
- Unit tests parse sample Python code and assert correct extraction:
  - Function definitions → `Function` symbols
  - Class definitions → `Class` symbols with `Method` children
  - `import` / `from ... import` → `Imports` edges
  - Function calls → `Calls` edges
  - Class inheritance → `Inherits` edges
- Edge source/target IDs are consistent with symbol IDs

---

## M4: Single-Repo Indexing + CLI

Wire parser and store into an end-to-end indexing pipeline. First usable binary.

**Scope:**
- `eagraph-cli`: binary crate, CLI argument parsing (`clap`)
- Config loading from `config.toml` (OS paths via `dirs` crate, `EAGRAPH_CONFIG` env override)
- Commands:
  - `eagraph index [--repo X]` — full parse + store pipeline
  - `eagraph status` — repo, branch, symbol/edge counts
  - `eagraph query <name> [--repo X]` — search symbols, print results
  - `eagraph config` — print resolved config path and contents
- File content hashing + skip-if-unchanged logic
- Include/exclude glob filtering from config
- Data directory layout: `data/{org}/{repo}/{branch}.db`

**Done when:**
- Point at a real Python repo in `config.toml`, run `eagraph index` → DB created with symbols and edges
- `eagraph status` shows correct counts
- `eagraph query <function_name>` finds and displays the symbol
- Re-running `eagraph index` on unchanged files skips them (observable via logs)
- `eagraph config` prints the resolved path

---

## M5: Retriever + MCP Server

The MCP interface that makes the graph useful to LLMs.

**Scope:**
- `eagraph-retriever`: `ContextRetriever` (single-repo mode), `SnippetReader`
  - `get_structural_context` — graph traversal + snippets
  - `get_dependents` — reverse traversal
  - `get_call_chain` — shortest path
- `eagraph-mcp`: JSON-RPC over stdio
  - Tool registration and dispatch
  - Tools: `get_context`, `get_file_symbols`, `search_symbols`, `get_dependents`, `get_call_chain`, `list_repos`
  - Response format matching DESIGN.md JSON structure
- `eagraph serve` command (stdio mode)
- `SnippetReader`: reads source lines from disk, configurable context lines

**Done when:**
- `eagraph serve` starts and accepts JSON-RPC over stdio
- Connectable via MCP inspector or Claude Code
- `get_context(repo, symbol, depth)` returns symbols with snippets and edges
- `search_symbols(query)` returns matching symbols
- `get_dependents(repo, file)` returns reverse dependencies
- `get_call_chain(from, to)` returns the path (or null if none)
- `list_repos()` returns indexed repos with status
- Snippets contain actual source lines from disk

---

## M6: File Watcher + Multi-Branch

Live indexing and branch-aware DB management.

**Scope:**
- `notify` watcher per repo with 200-500ms debounce
- Branch detection: periodic `git rev-parse --abbrev-ref HEAD`
- Per-branch DB files: `feature/auth` → `feature--auth.db`
- On branch switch: activate existing DB or create + full index
- On file change: hash → compare → re-parse → update DB (delete old + insert new in transaction)
- Watcher integrated into `eagraph serve`
- `eagraph prune [--repo X]` — delete branch DBs not accessed within `branch_ttl`

**Done when:**
- `eagraph serve` running, edit a watched file → symbols update within ~1s
- Switch git branch → new branch DB created and indexed automatically
- Switch back → previous branch DB reactivated instantly (no re-parse)
- `eagraph prune` deletes DBs older than configured `branch_ttl`
- Multiple repos watched simultaneously without interference

---

## M7: Cross-Repo Resolution

The multi-repo core: cross-repo edges, unresolved refs, and reconciliation.

**Scope:**
- `eagraph-crossref`: reconciliation engine
- `SqliteCrossRefStore` implementing `CrossRefStore`
- `sql/crossref_schema.sql` + all crossref query `.sql` files
- `[[deps]]` config parsing: package → repo mapping
- Forward resolution: after indexing, resolve outgoing imports against target repos
- Reverse reconciliation: after indexing, resolve other repos' unresolved refs pointing here
- Edge invalidation: demote to unresolved when target symbols are deleted/renamed
- MCP tools: `get_cross_repo_impact`, `list_deps`, `list_unresolved`
- CLI: `eagraph deps add/remove/list/check`
- `eagraph init` — interactive dep mapping (scan unresolved packages, prompt user)

**Done when:**
- Two repos configured with `[[deps]]` mapping, both indexed → cross-repo edges created
- `get_cross_repo_impact(repo, symbol)` returns consumers from other repos
- Delete a symbol in the target repo, re-index → edge demoted to unresolved, visible in `list_unresolved`
- Indexing order doesn't matter: A-then-B and B-then-A produce identical cross-repo edges
- `eagraph deps list` shows mappings with resolution counts
- `eagraph deps check` shows unresolved refs with actionable suggestions
- `eagraph init` scans and prompts for unmapped packages

---

## M8: Additional Languages

TypeScript and Rust extractors. The parser becomes multi-language.

**Scope:**
- TypeScript extractor: functions, classes, interfaces, imports, calls, type references
- Rust extractor: functions, structs, enums, impl blocks, use statements, trait implementations, method calls
- Language detection from file extension
- Dynamic grammar loading (tree-sitter grammars)

**Done when:**
- Unit tests for each extractor cover language-specific constructs:
  - TypeScript: arrow functions, class methods, `import { X } from`, JSX component usage, type/interface references
  - Rust: `fn`, `struct`, `enum`, `impl Trait for Struct`, `use crate::`, method calls on types
- End-to-end: index repos with mixed languages, `search_symbols` returns results across all languages
- Existing Python tests still pass

---

## M9: Enrichers

Post-indexing hooks that attach metadata to symbols.

**Scope:**
- Enricher pipeline wiring: after parse+store, run each registered enricher
- `GitBlameEnricher`: `last_author`, `last_modified`, `commit` per symbol (from `git blame`)
- `CodeownersEnricher`: parse `CODEOWNERS` file, match file paths to owners
- Annotations stored in `annotations` table via `GraphStore::upsert_annotations`
- Annotations included in MCP responses when present
- Config: enrichers opt-in, zero overhead when disabled

**Done when:**
- Index a repo with git history → `GitBlameEnricher` populates annotations
- Index a repo with `CODEOWNERS` → `CodeownersEnricher` populates owner annotations
- MCP `get_context` response includes `annotations` field with enricher data
- Disable enrichers in config → no annotations field in responses, no git/CODEOWNERS access

---

## M10: Embeddings

Semantic search over code via vector embeddings.

**Scope:**
- `EmbeddingStore` implementation (sqlite-vec or similar)
- Embedding pipeline: symbol → snippet extraction → embedding model → vector store
- Local model support: onnxruntime with `all-MiniLM-L6-v2`
- MCP tools (dynamically registered only when enabled):
  - `semantic_search(query, k)` — natural language search over code
  - `smart_context(query, symbol?)` — combined structural + semantic retrieval
- Combined retrieval: merge graph neighbors with embedding nearest-neighbors, dedup, rank
- Config: `[embeddings]` section, `enabled = true/false`

**Done when:**
- Enable embeddings in config, index a repo → embeddings generated and stored
- `semantic_search("data validation")` returns relevant symbols ranked by similarity
- `smart_context` merges graph traversal and embedding results without duplicates
- Disable embeddings → `semantic_search` and `smart_context` tools do not appear in MCP tool list
- Re-indexing a file updates its embeddings
