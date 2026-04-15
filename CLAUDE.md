# eagraph

Multi-repo code knowledge graph. Tree-sitter parsing, SQLite storage, CLI + Claude Code skill.

## Architecture

See DESIGN.md for full spec. See MILESTONES.md for implementation roadmap.

## Project structure

```
eagraph-core           → domain types, traits, RawEdge resolution
eagraph-store-sqlite   → SqliteGraphStore, FK on edges.source + edges.target (20 tests)
eagraph-parser         → generic tree-sitter extractor, dynamic .so grammar loading (10 tests)
eagraph-crossref       → cross-repo resolution (stub)
eagraph-retriever      → get_context, get_dependents, snippet reader
eagraph-mcp            → MCP server (stub)
eagraph-cli            → all commands (2 e2e tests)
grammars/              → .scm + .toml for 19 languages, registry.toml for grammar repos
skill/                 → Claude Code skill (symlink to ~/.claude/skills/eagraph)
tests/fixtures/
  grammars-src/        → vendored C sources for test .so compilation
  sample-repo/         → Python fixture project
```

## Current state

**M5 complete** (retriever + CLI + skill + viz). Next: M6 (file watcher + MCP).

## CLI commands

```
eagraph init <org>                         # create config
eagraph add <path> [--name X]              # add repo, detect languages, auto-index
eagraph index <repo> [--force] [--all]     # index repo(s)
eagraph status                             # repo/branch/symbol counts
eagraph query <name> [--repo X]            # search symbols
eagraph context <name> --repo X [--depth]  # symbol neighborhood + snippets
eagraph dependents <file> --repo X         # reverse impact analysis
eagraph symbols <file> --repo X            # file table of contents
eagraph chain <from> <to> --repo X         # shortest call path
eagraph viz                                # interactive graph in browser
eagraph config                             # print config path
eagraph grammars add <lang>...             # compile + install grammar
eagraph grammars list                      # show installed/available
eagraph grammars check                     # recommend grammars for repos
```

All query commands support `--json` global flag.

## Key conventions

- **Grammars**: .so/.dylib loaded at runtime via `libloading`. Zero compiled-in grammars.
- **Grammar config**: `grammars/registry.toml` maps names to GitHub repos. `.scm` + `.toml` per language.
- **Edge resolution**: extractor produces `RawEdge` (target is a name string). Indexer resolves to `Edge` (target is a SymbolId) via exact name match. `self.method()` resolves to `ClassName.method` using AST scope. Unresolvable edges dropped.
- **FK**: `edges.source` and `edges.target` both reference `symbols(id)`. Two-pass write: symbols first, edges second.
- **Auto-refresh**: every query command checks file mtimes before returning. Stale files re-indexed automatically. Zero manual re-indexing needed.
- **Git required**: `eagraph add` rejects non-git repos. Branch detection, `.gitignore`, and change tracking depend on git.
- **Indexing**: parallel parsing (rayon), single-transaction batch writes. `--force` deletes DB.
- **SQL**: `.sql` files embedded via `include_str!`
- **Storage**: OS app directory (`dirs` crate), never inside repos
- **DB layout**: `data/{org}/{repo}/{branch}.db`
- **File collection**: respects `.gitignore` via `ignore` crate
- **Test grammars**: build.rs compiles vendored C → .so, tests load via dlopen

## Build and test

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Add relevant tests after every implementation. Every new feature, bug fix, or behavior change gets a test.

## Warnings policy

Warnings are delayed errors. Treat `cargo clippy -D warnings` and `cargo fmt --check` as blocking gates on every change, not as optional cleanup. Before declaring any task done, run both and fix anything they surface. Do not defer warning cleanup to a later pass. If a lint is a genuine false positive, suppress it locally with `#[allow(...)]` and a one-line comment explaining why — do not weaken the workspace gate.
