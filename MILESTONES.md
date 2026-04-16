# eagraph: Implementation Milestones

Each milestone builds on the previous and is independently testable.

## Current state

**M1 through M5 are shipped.** The CLI, skill, agent, and interactive visualizer are all usable today against indexed repos. Post-M5 hardening (below) is ongoing and is what v1.0.0 was cut from.

**M6 through M9 are not started.** `eagraph-crossref` and `eagraph-mcp` are empty stub crates. No file watcher, no MCP server, no cross-repo edges, no enrichers, no embeddings.

---

## M1: Workspace + Core Types [DONE]

Cargo workspace, all crate stubs, `eagraph-core` with domain types and traits.

---

## M2: SQLite Store [DONE]

`SqliteGraphStore` implementing `GraphStore`. SQL in `.sql` files. WAL mode.

---

## M3: Parser [DONE]

Generic tree-sitter extractor driven by `.scm` query files. Dynamic grammar loading via `libloading` (`.so`/`.dylib` at runtime). `RawEdge` with target name strings, resolved to `Edge` via language-scoped name lookup. Grammar `.scm` + `.toml` configs bundled for the supported languages. `eagraph grammars add/list/check` commands.

---

## M4: Indexing + CLI [DONE]

Parallel parsing (rayon), batch transaction writes, edge resolution scoped to language via `ext_to_lang`, FK on both source and target. `.gitignore`-aware file collection. `eagraph init/add/index/status/query/config` commands. `--force` deletes the branch DB. Auto-index on `add` with grammar recommendations.

---

## M5: Retriever + CLI + Skill + Viz [DONE]

`eagraph-retriever`: `get_context`, `get_dependents`, snippet reader. CLI commands: `context`, `dependents`, `symbols`, `chain`, `viz`. Global `--json` flag. Claude Code skill in `skill/` and subagent in `agent/`. Interactive HTML graph visualization with d3-force and an embedded web server (Kind + Language dropdown filters, search with dimming, file/symbol view toggle, click-to-highlight). Auto-refresh before every query via mtime comparison. Git repo required for `add`.

---

## Post-M5 Hardening [ONGOING]

Work after M5 that tightens quality and ships a distributable binary.

- Zero clippy warnings gated by `cargo clippy -D warnings`.
- No silent fallbacks: removed `.unwrap_or(default)` / `.ok()?` / `let _ = fallible()` patterns across the tree. Non-UTF-8 paths bail at the store boundary via `eagraph_core::path_to_str`. Corrupt DB rows surface `rusqlite::Error::FromSqlConversionFailure` instead of defaulting to `SymbolKind::Variable`. Branch detection failures either propagate or are logged per repo, never silently substituted.
- `FromStr` on `SymbolKind` and `EdgeKind` with `Err = String` carrying the bad input.
- `&PathBuf` narrowed to `&Path` across function signatures; 27 redundant `.map_err(|e| anyhow::anyhow!(...))` calls removed since `anyhow::Error: From<EagraphError>` is auto-implemented.
- `CLAUDE.md` codifies clippy + fmt as blocking gates.
- `.github/workflows/ci.yml`: split lint job (fmt + clippy on ubuntu) and test matrix (ubuntu + macos) gated on lint.
- `.github/workflows/release.yml`: tag-triggered build for `x86_64-unknown-linux-gnu` and `aarch64-apple-darwin`, packaged with SHA256 sidecars, uploaded via `softprops/action-gh-release@v2`. Tarballs bundle `eagraph`, `README.md`, `skill/`, `agent/`.
- `scripts/release.sh`: takes explicit `X.Y.Z` (no bump shortcuts), validates strict version monotonicity, bumps `Cargo.toml`, snapshots everything in the working tree (`.gitignore` is the exclusion list), commits, tags, pushes both.
- `[profile.release]` uses `strip = true` and `lto = "thin"`. Binary size ~6 MB.

---

## M6: File Watcher + MCP Server [NOT STARTED]

Live indexing and MCP protocol for LLM tool use. `eagraph-crossref` and `eagraph-mcp` are empty stub crates today.

**Scope:**
- `notify` watcher per repo with debouncing.
- Branch detection via `git rev-parse --abbrev-ref HEAD` on change events.
- Per-branch DB management, branch switch activates or creates the right DB.
- `eagraph serve` (MCP over stdio).
- MCP tool registration: `get_context`, `search_symbols`, `get_dependents`, `get_file_symbols`, `get_call_chain`.
- `eagraph prune [--repo X]` to delete stale branch DBs.

**Done when:**
- `eagraph serve` accepts JSON-RPC over stdio and is connectable from Claude Code.
- File changes trigger re-index within ~1s.
- Branch switch creates or activates the right DB.
- `eagraph prune` cleans up branch DBs older than `branch_ttl`.

---

## M7: Cross-Repo Resolution [NOT STARTED]

Cross-repo edges, `[[deps]]` config, reconciliation engine, `crossref.db`. See DESIGN.md §5 for the intended design.

**Done when:**
- Two repos with a `[[deps]]` mapping produce cross-repo edges in `crossref.db`.
- Order of indexing doesn't matter (reverse reconciliation runs when any repo finishes indexing).
- `eagraph deps add/remove/list/check` commands work.

---

## M8: Enrichers [NOT STARTED]

Post-indexing hooks: `GitBlameEnricher`, `CodeownersEnricher`. Annotations in query responses.

**Done when:**
- Annotations populated from git history and CODEOWNERS.
- Disabling enrichers means zero overhead and no `annotations` field in responses.

---

## M9: Embeddings [NOT STARTED]

`EmbeddingStore`, local model (onnxruntime), `semantic_search` and `smart_context` commands.

**Done when:**
- `semantic_search("data validation")` returns relevant symbols.
- Combined structural and semantic retrieval works.
- Disabling embeddings means semantic commands don't appear in the CLI or MCP tool list.
