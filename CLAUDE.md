# eagraph

Multi-repo code knowledge graph served via MCP. Headless Rust server, tree-sitter parsing, SQLite storage.

## Architecture

See DESIGN.md for full spec. See MILESTONES.md for implementation roadmap.

## Project structure

```
eagraph-core           → domain types, traits (GraphStore, CrossRefStore, EmbeddingStore, Enricher)
eagraph-store-sqlite   → SqliteGraphStore (20 tests)
eagraph-parser         → generic tree-sitter extractor, dynamic grammar loading (10 tests)
eagraph-crossref       → cross-repo resolution (stub)
eagraph-retriever      → ContextRetriever, SnippetReader (stub)
eagraph-mcp            → MCP server, tool definitions (stub)
eagraph-cli            → binary: index, status, query, config commands (2 e2e tests)
grammars/              → .scm + .toml per language (checked in, no .so)
tests/fixtures/
  grammars-src/        → vendored C sources for compiling test .so files
  sample-repo/         → Python fixture project
```

Only `eagraph-cli` knows about concrete implementations. Everything else codes against traits.

## Current state

**M4 complete.** Next: M5 (retriever + MCP server).

Parser is fully data-driven: grammars loaded as .so/.dylib at runtime via `libloading`. Zero language-specific Rust code or crate dependencies. Adding a language = 3 files (.so + .scm + .toml) in the grammars directory, no recompilation.

## Key conventions

- **Grammars**: loaded from disk at runtime (`LanguageRegistry::from_dir`). No compiled-in grammars.
- **SQL**: lives in `.sql` files under `eagraph-store-sqlite/sql/`, embedded via `include_str!`
- **Edges table**: no FK constraints (edges can reference symbols in other files)
- **Storage**: config/data in OS app directory (`dirs` crate), never inside repos
- **DB layout**: one SQLite DB per repo per branch: `data/{org}/{repo}/{branch}.db`
- **Branch names**: `/` → `--` (e.g. `feature/auth` → `feature--auth.db`)
- **Method names**: class-qualified (`ClassName.method_name`)
- **Test grammars**: build.rs compiles C sources from `tests/fixtures/grammars-src/` into .so, tests load them via dlopen (same path as production)

## Commands

```
cargo check --workspace              # verify everything compiles
cargo test --workspace               # run all tests (32 total)
cargo run -p eagraph-cli -- index    # index repos from config
cargo run -p eagraph-cli -- status   # show repo/branch/symbol counts
cargo run -p eagraph-cli -- query X  # search for symbol by name
cargo run -p eagraph-cli -- config   # print config and grammars paths
```

Config: `--config` flag > `EAGRAPH_CONFIG` env > OS default.
Grammars: `EAGRAPH_GRAMMARS` env > `~/.config/eagraph/grammars/`.
