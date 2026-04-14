# eagraph

A code knowledge graph that spans multiple repositories. Point it at your repos, and it builds a graph of symbols (functions, classes, methods, imports) and their relationships (calls, inheritance, imports). Query it from the CLI now, from an MCP server soon.

Written in Rust. Uses tree-sitter for parsing, SQLite for storage. Doesn't touch your repos — all data lives in the OS application directory.

## What works today

- **Index any language** — grammars are loaded dynamically from shared libraries (.so/.dylib). No recompilation needed to add a language.
- **Query symbols** — substring search with optional kind filter, scoped to one or all repos
- **Graph traversal** — neighbor discovery with configurable depth (outgoing, incoming, both), shortest path between symbols
- **Change detection** — re-indexing skips files that haven't changed (SHA256 content hash)
- **Branch awareness** — one SQLite DB per branch, auto-detected via `git rev-parse`

## Getting started

### 1. Build

```
cargo build -p eagraph-cli
```

### 2. Install grammars

eagraph loads tree-sitter grammars at runtime. Install them with:

```bash
eagraph grammars add python typescript rust go
```

This clones each grammar's repo, compiles it into a shared library, and places the `.so`/`.dylib` + `.scm` + `.toml` into the grammars directory. Requires a C compiler (`cc`).

See what's available and what's installed:

```bash
eagraph grammars list
```

Grammars directory: `~/.config/eagraph/grammars/` (override with `EAGRAPH_GRAMMARS`).

#### Manual grammar installation

If a grammar isn't in the built-in list, or you want to customize the build:

```bash
# 1. Clone the grammar repo
git clone https://github.com/tree-sitter/tree-sitter-python
cd tree-sitter-python/src

# 2. Compile to shared library
#    macOS:
cc -shared -dynamiclib -fPIC -O2 -I. parser.c scanner.c -o python.dylib
#    Linux:
cc -shared -fPIC -O2 -I. parser.c scanner.c -o python.so

# Some grammars have scanner.c or scanner.cc — include it if present.
# If scanner.cc (C++), use c++ instead of cc.

# 3. Place in grammars directory
cp python.dylib ~/.config/eagraph/grammars/
```

You also need a `.toml` and `.scm` file alongside the shared library:

**python.toml** — which file extensions this grammar handles:
```toml
extensions = ["py", "pyi"]
module_separator = "."
```

**python.scm** — tree-sitter query patterns that tell eagraph what to extract:
```scheme
(function_definition name: (identifier) @func.name) @func.def
(class_definition name: (identifier) @class.name) @class.def
(call function: (identifier) @call.name) @call.def
; ... see grammars/python.scm for the full file
```

The capture names (`@func.def`, `@class.name`, `@call.name`, etc.) follow a convention the generic extractor understands. Use the [tree-sitter playground](https://tree-sitter.github.io/tree-sitter/playground) to explore a language's AST and write queries.

### 3. Configure repos

Create a config file at `~/.config/eagraph/config.toml` (Linux) or `~/Library/Application Support/eagraph/config.toml` (macOS):

```toml
[organization]
name = "myorg"

[[repos]]
name = "my-project"
root = "/path/to/my-project"
include = ["**/*.py"]
exclude = ["**/test_*", "**/__pycache__/**"]
```

### 4. Index and query

```
eagraph index                    # parse and store all configured repos
eagraph status                   # show repos, branches, symbol counts
eagraph query process_document   # find symbols by name
eagraph query validate --repo X  # search within one repo
eagraph config                   # print resolved config and grammars paths
```

Config path override: `EAGRAPH_CONFIG=/path/to/config.toml` or `--config`.
Grammars path override: `EAGRAPH_GRAMMARS=/path/to/grammars/`.

## Running tests

```
cargo test --workspace
```

The build script compiles a Python grammar `.so` from vendored C sources in `tests/fixtures/grammars-src/python/`, then the tests load it via `dlopen` — exercising the same dynamic loading path as production.

Run just the end-to-end test:

```
cargo test -p eagraph-cli --test e2e_index
```

The fixture repo at `tests/fixtures/sample-repo/` must be a git repository. If tests fail with a git error, run `git init && git add -A && git commit -m "init"` inside that directory.

## Project layout

```
crates/
  eagraph-core/               types, traits, config, errors
  eagraph-store-sqlite/       GraphStore implementation + SQL files
  eagraph-parser/             generic tree-sitter extractor + dynamic grammar loading
  eagraph-cli/                binary (index, status, query, config)
  eagraph-crossref/           cross-repo resolution (not yet implemented)
  eagraph-retriever/          context retriever (not yet implemented)
  eagraph-mcp/                MCP server (not yet implemented)
grammars/                     .scm query files + .toml configs (checked in)
tests/fixtures/
  sample-repo/                Python fixture project for e2e tests
  grammars-src/               vendored C sources for compiling test .so files
```

## Adding a language

For languages in the built-in list (see `eagraph grammars list`):

```bash
eagraph grammars add go
```

For anything else, follow the manual installation steps in the "Install grammars" section above. Any language with a [tree-sitter grammar](https://tree-sitter.github.io/tree-sitter/#parsers) works — you just need to compile the `.so`, write a `.scm` query file, and write a `.toml` config.

## What's next

MCP server over stdio, so an LLM can query the graph directly. Then file watching for live re-indexing, and cross-repo edge resolution. See `MILESTONES.md` for the full plan.
