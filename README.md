# eagraph

A code knowledge graph that spans multiple repositories. Point it at your repos, and it builds a graph of symbols (functions, classes, methods, imports) and their relationships (calls, inheritance, imports). Query from the CLI, explore with an interactive visualization, or let Claude Code use it directly via the bundled skill.

Written in Rust. Uses tree-sitter for parsing, SQLite for storage. All data lives in the OS application directory, never inside your repos.

## Quick start

```bash
# Build
cargo build --release -p eagraph-cli
cp target/release/eagraph ~/.local/bin/  # or wherever

# Install grammars for your languages
eagraph grammars add python typescript rust go

# Set up
eagraph init myorg
eagraph add /path/to/my-project

# Query
eagraph query MyClass
eagraph context MyClass
eagraph dependents src/models.py
eagraph symbols src/models.py
eagraph chain function_a function_b

# Visualize
eagraph viz
```

`eagraph add` detects languages, recommends missing grammars, and indexes immediately. Repos must be git repositories.

## Commands

All query commands support `--json` for structured output. `--repo` auto-detects from the current directory.

| Command | What it does |
|---|---|
| `eagraph init <org>` | Create config file |
| `eagraph add <path> [--name X]` | Add repo, detect languages, auto-index |
| `eagraph index <repo> [--force] [--all]` | Index repo(s) |
| `eagraph status` | Show repos, branches, symbol counts |
| `eagraph query <name>` | Search symbols by name |
| `eagraph context <symbol> [--depth N]` | Symbol neighborhood + source snippets |
| `eagraph dependents <file> [--depth N]` | What depends on this file |
| `eagraph symbols <file>` | File table of contents |
| `eagraph chain <from> <to>` | Shortest call path between two symbols |
| `eagraph viz [--port N]` | Interactive graph in browser |
| `eagraph config` | Print config and grammars paths |
| `eagraph grammars add <lang>...` | Compile and install grammar |
| `eagraph grammars list` | Show installed/available |
| `eagraph grammars check` | Recommend grammars for repos |

Data is always fresh. Every query checks file mtimes and re-indexes stale files automatically.

## Installing grammars

```bash
eagraph grammars add python typescript rust go java
```

This clones each grammar's repo, compiles it to a shared library, and installs it. Requires a C compiler (`cc`).

See all available grammars: `eagraph grammars list`

For unlisted grammars, build manually:

```bash
git clone https://github.com/tree-sitter/tree-sitter-python
cd tree-sitter-python/src

# macOS
cc -shared -dynamiclib -fPIC -O2 -I. parser.c scanner.c -o python.dylib

# Linux
cc -shared -fPIC -O2 -I. parser.c scanner.c -o python.so
```

Then place the `.so`/`.dylib` alongside a `.scm` (query patterns) and `.toml` (extensions config) in the grammars directory. See `grammars/python.scm` and `grammars/python.toml` for examples.

## Claude Code integration

eagraph ships with a skill and an agent for Claude Code.

```bash
# Install skill (makes /eagraph command available)
ln -s /path/to/eagraph/skill ~/.claude/skills/eagraph

# Install agent (subagents use eagraph for code navigation)
ln -s /path/to/eagraph/agent/eagraph-explorer.md ~/.claude/agents/eagraph-explorer.md
```

The skill teaches Claude to use `eagraph context`, `eagraph symbols`, etc. instead of doing multiple grep/glob/read calls. The agent ensures subagents also prefer eagraph for code exploration.

## Running tests

```bash
cargo test --workspace    # 32 tests
```

The build script compiles a Python grammar `.so` from vendored C sources, then tests load it via `dlopen` to exercise the same dynamic loading path as production.

## Project layout

```
crates/
  eagraph-core/             types, traits, config, errors
  eagraph-store-sqlite/     GraphStore + SQL files
  eagraph-parser/           generic tree-sitter extractor, dynamic grammar loading
  eagraph-retriever/        context retriever, snippet reader
  eagraph-cli/              binary with all commands
  eagraph-crossref/         cross-repo resolution (stub)
  eagraph-mcp/              MCP server (stub)
grammars/                   .scm + .toml for 19 languages, registry.toml
skill/                      Claude Code skill
agent/                      Claude Code subagent definition
tests/fixtures/
  grammars-src/             vendored C sources for test .so compilation
  sample-repo/              Python fixture project
```
