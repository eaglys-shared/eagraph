# eagraph — Implementation Milestones

Each milestone builds on the previous and is independently testable.

---

## M1: Workspace + Core Types [DONE]

Cargo workspace, all crate stubs, `eagraph-core` with domain types + traits.

---

## M2: SQLite Store [DONE]

`SqliteGraphStore` implementing `GraphStore`. SQL in `.sql` files. WAL mode. 20 unit tests.

---

## M3: Parser [DONE]

Generic tree-sitter extractor driven by `.scm` query files. Dynamic grammar loading via `libloading` (`.so`/`.dylib` at runtime). `RawEdge` with target name strings. 19 language `.scm` + `.toml` configs. `eagraph grammars add/list/check` commands. 10 unit tests.

---

## M4: Indexing + CLI [DONE]

Parallel parsing (rayon), batch transaction writes, edge resolution (name → SymbolId), FK on both source and target. `.gitignore`-aware file collection. `eagraph init/add/index/status/query/config` commands. `--force` deletes DB for fresh schema. Auto-index on `add` with grammar recommendations. 2 e2e tests.

---

## M5: Retriever + CLI + Skill + Viz [DONE]

`eagraph-retriever`: `get_context`, `get_dependents`, snippet reader. CLI commands: `context`, `dependents`, `symbols`, `chain`, `viz`. Global `--json` flag. Claude Code skill in `skill/`. Interactive HTML graph visualization with d3-force.

---

## M6: File Watcher + MCP Server

Live indexing and MCP protocol for LLM tool use.

**Scope:**
- `notify` watcher per repo with debouncing
- Branch detection via `git rev-parse --abbrev-ref HEAD`
- Per-branch DB management, branch switch detection
- `eagraph serve` command (MCP over stdio)
- MCP tool registration: get_context, search_symbols, get_dependents, etc.
- `eagraph prune [--repo X]` — delete stale branch DBs

**Done when:**
- `eagraph serve` accepts JSON-RPC over stdio, connectable from Claude Code
- File changes trigger re-index within ~1s
- Branch switch creates/activates the right DB
- `eagraph prune` cleans up old branch DBs

---

## M7: Cross-Repo Resolution

Cross-repo edges, `[[deps]]` config, reconciliation engine, `crossref.db`.

**Done when:**
- Two repos with `[[deps]]` mapping produce cross-repo edges
- Order of indexing doesn't matter
- `eagraph deps add/remove/list/check` commands work

---

## M8: Enrichers

Post-indexing hooks: `GitBlameEnricher`, `CodeownersEnricher`. Annotations in query responses.

**Done when:**
- Annotations populated from git history and CODEOWNERS
- Disabling enrichers = zero overhead

---

## M9: Embeddings

`EmbeddingStore`, local model (onnxruntime), `semantic_search` and `smart_context` commands.

**Done when:**
- `semantic_search("data validation")` returns relevant symbols
- Combined structural + semantic retrieval works
- Disabling embeddings = semantic commands don't appear
