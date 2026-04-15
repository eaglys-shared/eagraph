# eagraph - Server Architecture

## Overview

A headless Rust server that builds and serves a **multi-repo** code knowledge graph via MCP. The graph is organization-scoped, spanning multiple repositories with cross-repo edge resolution. Optionally wrapped in a Tauri shell for configuration and visualization.

### Data Hierarchy

```
Organization
  └── Repo A
  │     └── Branch: main
  │     │     └── Symbols, Edges (intra-repo)
  │     └── Branch: feature-auth
  │           └── Symbols, Edges (intra-repo)
  └── Repo B
  │     └── Branch: main
  │           └── Symbols, Edges (intra-repo)
  └── Cross-repo Edges (scoped to branch pairs)
  └── Unresolved Cross-repo Refs (pending resolution)
```

One SQLite DB per repo per branch, stored in the OS application data directory (not inside any repo). Cross-repo edges and unresolved refs live in a separate DB per org (`crossref.db`). Each repo gets its own file watcher that detects branch switches via `git rev-parse --abbrev-ref HEAD`.

## Components

### 1. Parser Engine

Responsible for extracting symbols and relationships from source files using tree-sitter.

- **Input**: file path + raw source
- **Output**: list of `Symbol` and `Edge` structs
- **Language support**: load tree-sitter grammars dynamically (start with Python, TypeScript, Rust)
- **Per-language extractors**: each language gets a small module that maps tree-sitter node types to symbol kinds (function, class, import, call, type reference, etc.)

```rust
struct Symbol {
    id: SymbolId,            // hash(file_path + name + kind)
    name: String,
    kind: SymbolKind,        // Function, Class, Method, Module, Variable, Type
    file_path: PathBuf,
    line_range: (u32, u32),
    metadata: Option<serde_json::Value>,
}

struct Edge {
    source: SymbolId,
    target: SymbolId,
    kind: EdgeKind,          // Calls, Imports, Inherits, References, TypeOf
}

struct CrossRepoEdge {
    source_repo: String,
    source_branch: String,
    source_symbol: SymbolId,
    target_repo: String,
    target_branch: String,
    target_symbol: SymbolId,
    kind: EdgeKind,
}

struct UnresolvedCrossRef {
    source_repo: String,
    source_branch: String,
    source_symbol: SymbolId,
    target_package: String,      // e.g. "shared_lib"
    target_path: String,         // e.g. "validators.validate_schema"
    kind: EdgeKind,
    created_at: u64,
}

struct RepoRecord {
    id: RepoId,
    name: String,
    root: PathBuf,
}

struct FileRecord {
    path: PathBuf,
    content_hash: String,
    last_indexed: u64,
}
```

Symbol IDs are scoped within a branch DB so they don't need repo or branch qualifiers. Cross-repo edges and unresolved refs are separate types stored in `crossref.db` with full repo+branch context.

### 2. Storage Traits

All persistence goes through traits. `GraphStore` and `EmbeddingStore` for storage, `Enricher` for post-indexing hooks.

```rust
trait GraphStore: Send + Sync {
    // Writes
    fn upsert_symbols(&self, symbols: &[Symbol]) -> Result<()>;
    fn upsert_edges(&self, edges: &[Edge]) -> Result<()>;
    fn delete_file_data(&self, file_path: &Path) -> Result<()>;
    fn upsert_file_record(&self, record: &FileRecord) -> Result<()>;

    // Reads
    fn get_symbol(&self, id: &SymbolId) -> Result<Option<Symbol>>;
    fn search_symbols(&self, query: &str, kind: Option<SymbolKind>) -> Result<Vec<Symbol>>;
    fn get_file_symbols(&self, file_path: &Path) -> Result<Vec<Symbol>>;
    fn get_file_record(&self, file_path: &Path) -> Result<Option<FileRecord>>;

    // Graph traversal (intra-repo only, within this branch DB)
    fn get_neighbors(&self, id: &SymbolId, direction: Direction, depth: u32) -> Result<SubGraph>;
    fn get_shortest_path(&self, from: &SymbolId, to: &SymbolId) -> Result<Option<Vec<SymbolId>>>;

    // Annotations (written by enrichers)
    fn upsert_annotations(&self, annotations: &[Annotation]) -> Result<()>;
    fn delete_annotations(&self, symbol_id: &SymbolId, source: &str) -> Result<()>;
    fn get_annotations(&self, symbol_id: &SymbolId) -> Result<Vec<Annotation>>;

    // Batch / lifecycle
    fn begin_transaction(&self) -> Result<Transaction>;
    fn commit(&self, tx: Transaction) -> Result<()>;
}

trait CrossRefStore: Send + Sync {
    // Resolved edges
    fn upsert_edges(&self, edges: &[CrossRepoEdge]) -> Result<()>;
    fn delete_edges_for(&self, repo: &str, branch: &str) -> Result<()>;
    fn get_edges_from(&self, repo: &str, branch: &str, symbol: &SymbolId) -> Result<Vec<CrossRepoEdge>>;
    fn get_edges_to(&self, repo: &str, branch: &str, symbol: &SymbolId) -> Result<Vec<CrossRepoEdge>>;
    fn get_edges_to_symbols(&self, repo: &str, branch: &str, symbols: &[SymbolId]) -> Result<Vec<CrossRepoEdge>>;

    // Unresolved refs
    fn upsert_unresolved(&self, refs: &[UnresolvedCrossRef]) -> Result<()>;
    fn delete_unresolved_for(&self, repo: &str, branch: &str) -> Result<()>;
    fn get_unresolved_targeting(&self, package: &str) -> Result<Vec<UnresolvedCrossRef>>;
    fn get_unresolved_from(&self, repo: &str, branch: &str) -> Result<Vec<UnresolvedCrossRef>>;
    fn delete_unresolved(&self, ids: &[UnresolvedCrossRefId]) -> Result<()>;
}

enum Direction {
    Outgoing,   // what does this symbol depend on
    Incoming,   // what depends on this symbol
    Both,
}

struct SubGraph {
    symbols: Vec<Symbol>,
    edges: Vec<Edge>,
}
```

```rust
trait EmbeddingStore: Send + Sync {
    fn upsert_embeddings(&self, items: &[(SymbolId, Vec<f32>)]) -> Result<()>;
    fn delete_embeddings(&self, ids: &[SymbolId]) -> Result<()>;
    fn search_nearest(&self, vector: &[f32], k: usize) -> Result<Vec<(SymbolId, f32)>>;
}
```

```rust
trait Enricher: Send + Sync {
    fn name(&self) -> &str;
    fn enrich_symbols(&self, symbols: &[Symbol]) -> Result<Vec<Annotation>>;
    fn enrich_file(&self, repo: &RepoRecord, file_path: &Path) -> Result<Vec<Annotation>>;
}

struct Annotation {
    symbol_id: SymbolId,
    source: String,     // enricher name, e.g. "git_blame", "codeowners"
    key: String,        // e.g. "last_author", "owner", "last_modified"
    value: String,      // the actual data
}
```

Enrichers are optional post-indexing hooks. After the core parse+store step, the system runs each registered enricher over the affected symbols. They write to a generic annotations table. No enrichers means no annotations, zero overhead.

Future enrichers: `GitBlameEnricher` (last author, commit, date per symbol), `CodeownersEnricher` (ownership from CODEOWNERS file), `TestCoverageEnricher` (is this symbol covered by tests).

`GraphStore` operates on a single branch DB. `CrossRefStore` operates on `crossref.db`. `EmbeddingStore` and `Enricher` are optional.

**Initial implementations:**

| Trait            | V1 Implementation       | Future options           |
|------------------|-------------------------|--------------------------|
| `GraphStore`     | `SqliteGraphStore`      | `CozoStore`, `Neo4jStore` |
| `CrossRefStore`  | `SqliteCrossRefStore`   | |
| `EmbeddingStore` | not wired               | `SqliteVecStore`, `UsearchStore`, `QdrantStore` |
| `Enricher`       | not wired               | `GitBlameEnricher`, `CodeownersEnricher` |

### 3. SQLite GraphStore Implementation

SQL lives in `.sql` files, not in Rust strings. Embedded at compile time via `include_str!` so they're still part of the binary, but editable and readable as standalone files.

```
eagraph-store-sqlite/
  sql/
    branch_schema.sql         # per-branch DB schema
    crossref_schema.sql       # crossref.db schema
    queries/
      upsert_symbol.sql
      upsert_edge.sql
      delete_file_data.sql
      search_symbols.sql
      get_neighbors.sql
      get_shortest_path.sql
      upsert_annotation.sql
      get_annotations.sql
    crossref_queries/
      upsert_edge.sql
      get_edges_from.sql
      get_edges_to.sql
      get_edges_to_symbols.sql
      delete_edges_for.sql
      upsert_unresolved.sql
      get_unresolved_targeting.sql
      get_unresolved_from.sql
      delete_unresolved.sql
```

```rust
mod sql {
    pub const BRANCH_SCHEMA: &str = include_str!("sql/branch_schema.sql");
    pub const CROSSREF_SCHEMA: &str = include_str!("sql/crossref_schema.sql");
    pub const UPSERT_SYMBOL: &str = include_str!("sql/queries/upsert_symbol.sql");
    pub const GET_NEIGHBORS: &str = include_str!("sql/queries/get_neighbors.sql");
    // ...
}
```

**branch_schema.sql** (used for each `{repo}/{branch}.db`):

```sql
CREATE TABLE symbols (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL,
    file_path   TEXT NOT NULL,
    line_start  INTEGER,
    line_end    INTEGER,
    metadata    TEXT
);

CREATE TABLE edges (
    source      TEXT NOT NULL REFERENCES symbols(id),
    target      TEXT NOT NULL REFERENCES symbols(id),
    kind        TEXT NOT NULL,
    PRIMARY KEY (source, target, kind)
);

CREATE TABLE files (
    path         TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    last_indexed INTEGER NOT NULL
);

CREATE TABLE annotations (
    symbol_id   TEXT NOT NULL REFERENCES symbols(id),
    source      TEXT NOT NULL,
    key         TEXT NOT NULL,
    value       TEXT NOT NULL,
    PRIMARY KEY (symbol_id, source, key)
);

CREATE INDEX idx_edges_source ON edges(source);
CREATE INDEX idx_edges_target ON edges(target);
CREATE INDEX idx_symbols_file ON symbols(file_path);
CREATE INDEX idx_symbols_name ON symbols(name);
CREATE INDEX idx_annotations_symbol ON annotations(symbol_id);
```

**crossref_schema.sql** (one per org, `crossref.db`):

```sql
CREATE TABLE cross_edges (
    source_repo     TEXT NOT NULL,
    source_branch   TEXT NOT NULL,
    source_symbol   TEXT NOT NULL,
    target_repo     TEXT NOT NULL,
    target_branch   TEXT NOT NULL,
    target_symbol   TEXT NOT NULL,
    kind            TEXT NOT NULL,
    PRIMARY KEY (source_repo, source_branch, source_symbol,
                 target_repo, target_branch, target_symbol, kind)
);

CREATE TABLE unresolved_refs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    source_repo     TEXT NOT NULL,
    source_branch   TEXT NOT NULL,
    source_symbol   TEXT NOT NULL,
    target_package  TEXT NOT NULL,
    target_path     TEXT NOT NULL,
    kind            TEXT NOT NULL,
    created_at      INTEGER NOT NULL
);

CREATE INDEX idx_cross_source ON cross_edges(source_repo, source_branch, source_symbol);
CREATE INDEX idx_cross_target ON cross_edges(target_repo, target_branch, target_symbol);
CREATE INDEX idx_unresolved_package ON unresolved_refs(target_package);
CREATE INDEX idx_unresolved_source ON unresolved_refs(source_repo, source_branch);
```

**queries/get_neighbors.sql:**

```sql
WITH RECURSIVE reachable(id, depth) AS (
    SELECT target, 1 FROM edges WHERE source = :symbol_id
    UNION
    SELECT e.target, r.depth + 1
    FROM edges e JOIN reachable r ON e.source = r.id
    WHERE r.depth < :max_depth
)
SELECT s.* FROM symbols s
JOIN reachable r ON s.id = r.id;
```

Each query is a named `.sql` file with `:named` parameters. The Rust code loads them via `include_str!`, passes them to `rusqlite` with parameter binding. Swapping a query means editing a `.sql` file, not hunting through Rust code. If a future store backend (Cozo, etc.) needs different query syntax, it has its own `sql/` directory with its own files.

WAL mode enabled at connection init. Everything except `get_neighbors` and `get_shortest_path` is straightforward CRUD.

### 4. Embedding Strategy (Later Stage)

When implemented, embeddings serve a different retrieval path than the graph:

- **Graph traversal**: "give me the structural neighborhood of X" (precise, follows code dependencies)
- **Embedding search**: "find code that does something similar to this description" (fuzzy, semantic)

The MCP server composes both: graph traversal for known entry points, embedding search for natural language queries where the user doesn't know the symbol name.

**Embedding pipeline** (when wired):

```
symbol extracted
    -> snippet extractor reads source lines
    -> embedding model encodes snippet -> Vec<f32>
    -> EmbeddingStore::upsert_embeddings
```

Embedding model options: local (onnxruntime with a small model like `all-MiniLM-L6-v2`), or remote API call. Configurable.

The `EmbeddingStore` trait keeps this completely decoupled. You can swap sqlite-vec for usearch for qdrant without touching anything above the trait boundary.

### 5. Cross-Repo Edge Resolution

Cross-repo edges require explicit dependency declarations. The system does not guess, infer, or heuristically match imports to repos. Every cross-repo relationship flows from a user-declared `[[deps]]` entry in the config.

**The only mechanism:** a `[[deps]]` entry maps a package name to a repo.

```toml
[[deps]]
package = "shared_lib"
repo = "shared-lib"
```

This tells eagraph: when any repo imports from the `shared_lib` package, the target symbols live in the `shared-lib` repo. The parser already extracts import statements from the AST. The config tells it where those imports point. The resolver joins the two.

No import path pattern matching. No OpenAPI/protobuf/GraphQL contract parsing. No string similarity. No regex. If a cross-repo dependency isn't declared in `[[deps]]`, it doesn't exist in the graph.

**How resolution works:**

When the parser extracts an import like `from shared_lib.validators import validate_schema`, it checks the `[[deps]]` table for a matching package name. Two outcomes:

1. **Target repo is already indexed.** Look up `validate_schema` in the `shared-lib` repo's symbol table. If found, store a `CrossRepoEdge` in `crossref.db`. If not found (symbol doesn't exist, or the path is wrong), store an `UnresolvedCrossRef`.

2. **Target repo is not yet indexed.** Store an `UnresolvedCrossRef` with `target_package = "shared_lib"` and `target_path = "validators.validate_schema"`.

**Eager reconciliation:**

When any repo finishes indexing, the system runs a reconciliation pass from both directions:

- **Forward:** resolve this repo's outgoing unresolved refs against target repos that are already indexed.
- **Reverse:** check if any other repos have unresolved refs pointing at this repo (via `[[deps]]` package mappings), and attempt to resolve them against the newly indexed symbols.

```
repo B ("shared-lib") finishes indexing
  -> look up which packages map to "shared-lib" in [[deps]]
     -> finds: package = "shared_lib"
  -> SELECT * FROM unresolved_refs WHERE target_package = 'shared_lib'
  -> for each unresolved ref:
      -> parse target_path ("validators.validate_schema")
      -> search repo B's symbol table for "validate_schema" in file path containing "validators"
      -> if found: insert CrossRepoEdge, delete the UnresolvedCrossRef
      -> if not found: leave it (symbol doesn't exist yet, or path is wrong)
```

This is a SQL join, not a parse. The expensive work (tree-sitter) already happened. Reconciliation is bookkeeping that takes milliseconds even with thousands of unresolved refs.

**Order independence:** because reconciliation runs from both directions, the order in which repos are added and indexed does not matter. Whether you add repos A→B→C or C→A→B or in any other sequence, you converge on the same set of cross-repo edges.

**Interactive dependency setup:**

`eagraph init` collects unresolved external package names across all repos and asks the user to map them:

```
Found 3 external packages imported across your repos:

  shared_lib        (imported by: api-service, client-app)
  auth_sdk          (imported by: api-service)
  proto_types       (imported by: api-service, client-app, shared-lib)

Which repo provides each package? (leave blank to skip)

  shared_lib  → shared-lib
  auth_sdk    →
  proto_types → proto-types

Written 2 entries to [[deps]] in config.toml.
1 package skipped (auth_sdk). You can add it later with:
  eagraph deps add auth_sdk --repo <repo-name>
```

Autocomplete from repos already in the config. Skip means "I don't want this tracked." The tool writes the `[[deps]]` entries. No guessing.

**CLI for ongoing management:**

```
eagraph deps add <package> --repo <repo>    # add a dep mapping
eagraph deps remove <package>               # remove a dep mapping
eagraph deps list                           # show all mappings + resolution status
eagraph deps check                          # show unresolved refs across all repos
```

`eagraph deps check` is the ongoing version of the init-time prompt. Run it after adding a new repo to see what new unresolved externals appeared.

### 6. Context Retriever

This is the layer between storage and MCP. It manages multiple `GraphStore` handles (one per active repo+branch) and a single `CrossRefStore`. It doesn't know about SQLite or vectors.

```rust
struct ContextRetriever {
    stores: HashMap<(RepoName, BranchName), Box<dyn GraphStore>>,
    crossref: Box<dyn CrossRefStore>,
    embeddings: Option<Box<dyn EmbeddingStore>>,
    snippet_reader: SnippetReader,
}

impl ContextRetriever {
    fn get_structural_context(&self, repo: &str, branch: &str, symbol: &str, depth: u32) -> Result<ContextResult>;
    fn get_semantic_context(&self, query: &str, k: usize) -> Result<Option<ContextResult>>;
    fn get_combined_context(&self, repo: &str, branch: &str, symbol: &str, query: &str) -> Result<ContextResult>;
    fn get_dependents(&self, repo: &str, branch: &str, file: &Path, depth: u32) -> Result<ContextResult>;
    fn get_call_chain(&self, from: &str, to: &str) -> Result<Option<ContextResult>>;
    fn get_cross_repo_impact(&self, repo: &str, branch: &str, symbol: &str, depth: u32) -> Result<ContextResult>;
}

struct ContextResult {
    symbols: Vec<SymbolWithSnippet>,
    edges: Vec<Edge>,
    cross_repo_edges: Vec<CrossRepoEdge>,
    unresolved_refs: Vec<UnresolvedCrossRef>,   // surfaces what couldn't be resolved
    repos_involved: Vec<RepoId>,
    retrieval_method: RetrievalMethod,  // Structural, Semantic, Combined
}
```

`ContextResult` includes `unresolved_refs` so the MCP response can surface them. If a query returns unresolved refs, the LLM (or the user) knows there are declared dependencies that couldn't be matched to actual symbols. Either the target repo isn't indexed yet, or the import path doesn't match any symbol.

`get_combined_context` merges graph neighbors with embedding nearest-neighbors, deduplicates, and ranks. The merge strategy is a config knob.

### 7. File Watcher

- One `notify` watcher **per repo**, each tracking its current branch via periodic `git rev-parse --abbrev-ref HEAD`
- On branch switch: detect new branch name, activate (or create) the corresponding branch DB, no re-parse needed if DB already exists
- On file change within a branch: hash content, compare with stored record
- If changed: re-parse, update the active branch DB (delete old + insert new in one transaction), then run cross-repo reconciliation for affected symbols
- After re-indexing: invalidate cross-repo edges pointing at changed symbols in this repo (demote to unresolved if target symbol was deleted or renamed)
- On branch switch / git pull: batch re-index for that repo+branch
- Debounce: 200-500ms per watcher

### 8. MCP Server

JSON-RPC over stdio (for Claude Code / local tools) and SSE (for remote clients / Tauri).

**Tools always available:**

`get_context(repo, symbol_name, max_depth=3)`: Structural subgraph + snippets from ContextRetriever. Includes cross-repo edges when they exist.

`get_file_symbols(repo, file_path)`: All symbols in a file.

`search_symbols(query, kind?, repo?)`: Exact/fuzzy search over symbol names. Searches all repos if `repo` is omitted.

`get_dependents(repo, file_path, max_depth=2)`: Reverse traversal. "What breaks if I change this?" Crosses repo boundaries via `crossref.db`.

`get_call_chain(from_symbol, to_symbol)`: Shortest path between two symbols. Works across repos.

`get_cross_repo_impact(repo, symbol, depth=2)`: Like get_dependents but specifically surfaces cross-repo consumers. "What in other repos breaks if I change this?"

`list_repos()`: List all indexed repos, their branches, and index status.

`list_deps()`: List all `[[deps]]` mappings and their resolution status (how many resolved edges vs. unresolved refs per mapping).

`list_unresolved()`: List all unresolved cross-repo refs. Useful for debugging missing edges.

**Tools conditionally available (when EmbeddingStore is configured):**

`semantic_search(query, k=10)`: Natural language search over code snippets.

`smart_context(query, symbol?)`: Combined structural + semantic retrieval.

The MCP server registers tools dynamically based on what's available. No `EmbeddingStore` configured = semantic tools simply don't appear in the tool list.

**Response format:**

```json
{
  "repos_involved": ["api-service", "shared-lib"],
  "symbols": [
    {
      "name": "process_document",
      "kind": "function",
      "repo": "api-service",
      "file": "src/pipeline/processor.py",
      "lines": [45, 92],
      "snippet": "def process_document(doc: Document) -> DataMart:\n    ...",
      "relevance": { "method": "structural", "depth": 1 },
      "annotations": {
        "git_blame": { "last_author": "tanaka", "last_modified": "2026-04-10" },
        "codeowners": { "owner": "@backend-team" }
      }
    }
  ],
  "edges": [
    { "from": "api-service::process_document", "to": "shared-lib::validate_schema", "kind": "calls", "cross_repo": true }
  ],
  "unresolved": [
    { "from": "api-service::process_document", "target_package": "auth_sdk", "target_path": "auth.verify_token", "kind": "calls" }
  ]
}
```

The `annotations` field is only present when enrichers have produced data. No enrichers configured = no field, zero noise.

The `unresolved` field surfaces cross-repo refs that couldn't be matched to actual symbols. This tells the LLM (or the user) that there are known dependencies missing from the graph. Either the target repo isn't indexed, the `[[deps]]` mapping is missing, or the import path doesn't match any symbol.

### 9. Snippet Reader

Reads actual source lines from disk for symbols in a retrieval result. The graph tells us *where* to look, this component reads the content.

- Always reads from disk (source of truth, graph just has line ranges)
- Configurable surrounding context lines
- Respects `.gitignore` and config excludes

### 10. Config

Config and data live in the OS canonical application directory, not inside repositories. Resolved via the `dirs` crate.

```
macOS:   ~/Library/Application Support/eagraph/
Linux:   $XDG_CONFIG_HOME/eagraph/  (usually ~/.config/eagraph/)
Windows: %APPDATA%\eagraph\
```

Layout:

```
~/.config/eagraph/                          # or OS equivalent
  config.toml
  data/
    alchemista/                             # org
      api-service/                          # repo
        main.db
        feature-auth.db
        hotfix-payments.db
      shared-lib/                           # repo
        main.db
        develop.db
      client-app/                           # repo
        main.db
      crossref.db                           # cross-repo edges + unresolved refs
```

Each branch DB is a self-contained SQLite file with the full schema (symbols, edges, files, annotations). Branch name is sanitized for filesystem safety (`/` becomes `--`, e.g. `feature/auth` becomes `feature--auth.db`).

Branch lifecycle: new branch DB is created on first encounter. The daemon detects branch via `git rev-parse --abbrev-ref HEAD`. On switch, it activates the corresponding DB (or creates + indexes if new). Stale branch DBs are cleaned up based on `branch_ttl`.

Cross-repo edges and unresolved refs live in `crossref.db`, scoped by `(source_repo, source_branch, target_repo, target_branch)`. When resolving cross-repo queries, the retriever ATTACHes the relevant branch DBs alongside `crossref.db` into one SQLite connection.

Override config path with `--config /path/to/config.toml` or `EAGRAPH_CONFIG` env var for CI/server use.

**config.toml:**

```toml
[organization]
name = "alchemista"

[server]
mode = "stdio"  # or "sse"
port = 3100

[graph]
store = "sqlite"  # future: "cozo", "neo4j"
max_hop_depth = 4
branch_ttl = "30d"  # auto-delete branch DBs not accessed in this period

[embeddings]
enabled = false
# store = "sqlite-vec"
# model = "all-MiniLM-L6-v2"
# model_path = "./models/minilm.onnx"

[[repos]]
name = "api-service"
root = "/path/to/api-service"
include = ["src/**/*.py"]
exclude = ["**/test_*", "**/__pycache__/**"]

[[repos]]
name = "shared-lib"
root = "/path/to/shared-lib"
include = ["src/**/*.py"]

[[repos]]
name = "client-app"
root = "/path/to/client-app"
include = ["src/**/*.ts", "src/**/*.tsx"]
exclude = ["**/node_modules/**"]

[[deps]]
package = "shared_lib"
repo = "shared-lib"

[[deps]]
package = "proto_types"
repo = "proto-types"
```

Repos point to code directories but eagraph never writes anything inside them. No `.eagraph/`, no dotfiles, no pollution.

## Binary Modes

```
eagraph init                           # scaffold config, interactive dep mapping
eagraph serve                          # headless, MCP over stdio
eagraph serve --sse                    # headless, MCP over SSE
eagraph index                          # index all repos, current branches
eagraph index --repo api-service       # index a single repo, current branch
eagraph query "func_name"              # CLI query for debugging
eagraph query "func_name" --repo X     # scoped to one repo
eagraph status                         # show all repos, branches, symbol/edge counts
eagraph config                         # print resolved config path and contents
eagraph prune                          # delete branch DBs older than branch_ttl
eagraph prune --repo api-service       # prune a single repo
eagraph deps add <pkg> --repo <repo>   # add a dep mapping
eagraph deps remove <pkg>              # remove a dep mapping
eagraph deps list                      # show all mappings + resolution status
eagraph deps check                     # show unresolved refs across all repos
```

Same binary, different entry points. Tauri app spawns `eagraph serve --sse` as a sidecar.

## Tauri App (Separate Crate)

Thin client. Communicates with the server over SSE/HTTP.

- Organization overview: all repos, their index status, symbol/edge counts
- Repo management and config editing
- Live graph visualization with cross-repo edges highlighted (d3-force or similar)
- Symbol search with jump-to-file, filterable by repo
- "What would the LLM see for this symbol?" preview
- Cross-repo impact explorer
- Unresolved refs dashboard (what's missing, which repos need indexing)

Does NOT embed any graph or embedding logic.

## Project Structure

```
eagraph/
├── Cargo.toml                  # workspace
├── crates/
│   ├── eagraph-core/         # domain types, GraphStore + EmbeddingStore + Enricher traits
│   ├── eagraph-store-sqlite/ # SqliteGraphStore, SqliteCrossRefStore, SqliteVecStore (later)
│   │   └── sql/              # .sql files: schemas, queries
│   ├── eagraph-parser/       # tree-sitter engine, language extractors
│   ├── eagraph-crossref/     # cross-repo reconciliation engine
│   ├── eagraph-retriever/    # ContextRetriever, snippet reading, ranking
│   ├── eagraph-mcp/          # MCP protocol, tool definitions, server
│   ├── eagraph-cli/          # binary, CLI commands
│   └── eagraph-tauri/        # tauri app (optional)
├── grammars/
└── config.example.toml
```

## Dependency Flow

```
eagraph-core          (types, traits -- depends on nothing)
    ^
eagraph-store-sqlite  (implements traits)
eagraph-parser        (uses core types)
eagraph-crossref      (reconciliation engine, uses core traits)
    ^
eagraph-retriever     (uses traits, not implementations)
    ^
eagraph-mcp           (uses retriever)
    ^
eagraph-cli           (wires everything, picks implementations)
eagraph-tauri          (client only, talks to server over network)
```

Only `eagraph-cli` knows about concrete implementations. Everything above the trait boundary is swappable. Adding a new store backend = new crate, implement the trait, add a match arm in CLI's wiring code.

## Indexing Flow

```
file change detected (repo watcher identifies repo + current branch)
    -> resolve active branch DB: data/{org}/{repo}/{branch}.db
    -> hash content, compare with stored record in branch DB
    -> if unchanged, skip
    -> tree-sitter parse -> AST
    -> language extractor -> Vec<Symbol>, Vec<Edge>
    -> transaction on branch DB {
        GraphStore::delete_file_data(path)
        GraphStore::upsert_symbols(new_symbols)
        GraphStore::upsert_edges(new_edges)
        GraphStore::upsert_file_record(new_hash)
      }
    -> cross-repo resolution for affected symbols {
        for each import edge where package matches a [[deps]] entry:
            target_repo = deps[package].repo
            if target_repo is indexed:
                search target repo's symbol table for the import target
                if found: CrossRefStore::upsert_edges([resolved_edge])
                if not found: CrossRefStore::upsert_unresolved([ref])
            else:
                CrossRefStore::upsert_unresolved([ref])
      }
    -> reverse reconciliation {
        look up which [[deps]] packages map to THIS repo
        CrossRefStore::get_unresolved_targeting(each package)
        for each unresolved ref:
            search THIS repo's symbol table for the target
            if found: promote to CrossRepoEdge, delete UnresolvedCrossRef
      }
    -> cross-repo edge invalidation {
        collect symbol IDs from changed files in THIS repo
        CrossRefStore::get_edges_to_symbols(this_repo, this_branch, changed_symbol_ids)
        for each existing cross-repo edge pointing at a changed symbol:
            if target symbol still exists in THIS repo's DB with same ID: keep
            if target symbol is gone (deleted/renamed): demote to UnresolvedCrossRef
      }
    -> if embeddings enabled {
        generate embeddings for new symbols
        EmbeddingStore::delete_embeddings(old_ids)
        EmbeddingStore::upsert_embeddings(new_embeddings)
      }
    -> for each registered enricher {
        Enricher::enrich_symbols(new_symbols)
        GraphStore::upsert_annotations(results)
      }

branch switch detected (git rev-parse returns different branch name)
    -> if data/{org}/{repo}/{new_branch}.db exists:
        swap active DB handle, done (instant)
    -> else:
        create new branch DB
        full index into new DB
        run cross-repo resolution + reverse reconciliation
```

## Query Flow

```
MCP tool call: get_context("api-service", "main", "process_document", depth=3)
    -> ContextRetriever::get_structural_context
        -> resolve DB handle for api-service/main
        -> GraphStore::search_symbols("process_document")
        -> GraphStore::get_neighbors(id, Both, 3)
        -> CrossRefStore::get_edges_from("api-service", "main", id)
            -> for each cross-repo edge: look up target symbol in target repo's branch DB
        -> CrossRefStore::get_unresolved_from("api-service", "main")
            -> filter to refs originating from this symbol's neighborhood
        -> SnippetReader reads source for each symbol (from correct repo root)
    -> return ContextResult (includes both resolved edges and unresolved refs)

MCP tool call: get_cross_repo_impact("shared-lib", "main", "validate_schema", depth=2)
    -> ContextRetriever::get_cross_repo_impact
        -> CrossRefStore::get_edges_to("shared-lib", "main", id)
        -> for each incoming cross-repo edge:
            resolve source symbol from source repo's branch DB
        -> SnippetReader reads source
    -> return ContextResult

MCP tool call: semantic_search("data validation logic")
    -> ContextRetriever::get_semantic_context
        -> encode query via embedding model
        -> EmbeddingStore::search_nearest(vector, 10)  // searches across all branch DBs
        -> resolve each result to its repo+branch DB
        -> SnippetReader reads source
    -> return ContextResult

MCP tool call: list_unresolved()
    -> CrossRefStore::get_all_unresolved()
    -> group by target_package
    -> return summary with counts and actionable suggestions
       e.g. "auth_sdk: 12 unresolved refs. Add with: eagraph deps add auth_sdk --repo <repo>"
```

## What This Doesn't Do (Intentionally)

- **Full LSP**: tree-sitter gives 80% accuracy without needing compilation. Good enough for context retrieval.
- **Caching LLM responses**: out of scope. The MCP client handles that.
- **Embedding model training**: uses off-the-shelf models. Fine-tuning on your codebase is a future consideration, not a V1 concern.
- **Heuristic cross-repo resolution**: no guessing which repo a package belongs to. No regex matching on import paths. No OpenAPI/protobuf contract parsing. All cross-repo relationships are explicitly declared via `[[deps]]`. The system joins AST-extracted imports with user-declared mappings. Nothing in between.
