[English](README.md) | [日本語](README.ja.md)

<p align="center">
  <b>E A G R A P H</b>
</p>

<img width="1783" height="940" alt="image" src="https://github.com/user-attachments/assets/5273145f-6814-46da-9e65-34893526bd85" />

<p align="center">
  <sub><i>ALCHEMISTA Labs code graph</i></sub>
</p>

---

eagraph is a code knowledge graph designed to reduce the tokens Claude Code spends on navigation. When an agent needs to find callers, trace a call chain, or map a file's structure, it would otherwise chain several grep, glob, and read calls. eagraph answers the same question with a single query against a pre-built index. The project ships as a Claude Code skill, runs as a standalone CLI, and includes an interactive graph visualizer for browsing your codebase.
Written in Rust. Uses tree-sitter for parsing, SQLite for storage. All data lives in the OS application directory, never inside your repos.


Below is an example of a exploration of Tiny C Compiler source code, a fairly large codebase. The prompt is about tracing execution path of code compilation from source files to executable output. With `eagraph` skill (left part) and `eagraph-explorer` agent, it could dramatically cut the tool calling count, token usage, and exploration time. The result is also more precise by including line number of the function of interests. YMMV.

<img width="1041" height="425" alt="image" src="https://github.com/user-attachments/assets/ea58dfbf-89de-4c6d-a521-e7c038132d82" />
  




## Install

Download a precompiled binary from the [latest release](https://github.com/eaglys-shared/eagraph/releases/latest). Two targets are published per tag:

- `x86_64-unknown-linux-gnu`: Linux x86_64
- `aarch64-apple-darwin`: macOS Apple Silicon

Intel Mac builds are not published. Intel Mac users should build from source (below).

Each Release page lists the `.tar.gz` tarball and a sibling `.tar.gz.sha256` file per target. Download both, verify the archive, then install the binary:

```bash
# Download the tarball and its .sha256 sidecar from the Release page first.
tar -xzf eagraph-v<X.Y.Z>-<target>.tar.gz
shasum -a 256 -c eagraph-v<X.Y.Z>-<target>.tar.gz.sha256
sudo install eagraph-v<X.Y.Z>-<target>/eagraph /usr/local/bin/
```

### macOS: unsigned binary

The macOS builds are **not code-signed or notarized**. Gatekeeper will refuse to launch the binary on first run with an error similar to *"eagraph cannot be opened because the developer cannot be verified"* or *"the executable is damaged"*. Remove the quarantine attribute to allow it through:

```bash
xattr -d com.apple.quarantine /usr/local/bin/eagraph
```

One-time step. Apply it to the installed path. If you prefer to avoid the unsigned binary entirely, build from source (below).

### Build from source

Requires a stable Rust toolchain (install via [rustup](https://rustup.rs)).

```bash
cargo build --release -p eagraph-cli
sudo install target/release/eagraph /usr/local/bin/
```

## Using eagraph with Claude Code

eagraph is primarily used as a Claude Code skill. Once the CLI is on your PATH, copy the `skill/` and `agent/` directories into your Claude Code config. Both ship inside the release tarball alongside the binary; if you built from source, they are at the top of the repo checkout.

```bash
mkdir -p ~/.claude/skills ~/.claude/agents
cp -r skill ~/.claude/skills/eagraph
cp agent/eagraph-explorer.md ~/.claude/agents/
```

On upgrade, re-run these copies after extracting the new tarball (or pulling the new source). The skill teaches Claude to use `eagraph context`, `eagraph symbols`, etc. instead of chaining multiple grep/glob/read calls. The agent ensures subagents also prefer eagraph for code exploration.

If Claude Code documents different install paths for your platform or version, follow those. The commands above are the standard Unix locations.

## Using eagraph as a CLI

```bash
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

Two families: query commands (`query`, `context`, `dependents`, `symbols`, `chain`) are what the Claude Code skill invokes on the agent's behalf; the rest are admin and setup. All query commands support `--json` for structured output. `--repo` auto-detects from the current working directory.

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

## Grammars

```bash
eagraph grammars add python typescript rust go java
```

This clones each grammar's repo, compiles it to a shared library, and installs it. Requires a C compiler (`cc`).

See all available grammars: `eagraph grammars list`

<details>
<summary>Building an unlisted grammar manually</summary>

```bash
git clone https://github.com/tree-sitter/tree-sitter-python
cd tree-sitter-python/src

# macOS
cc -shared -dynamiclib -fPIC -O2 -I. parser.c scanner.c -o python.dylib

# Linux
cc -shared -fPIC -O2 -I. parser.c scanner.c -o python.so
```

Then place the `.so`/`.dylib` alongside a `.scm` (query patterns) and `.toml` (extensions config) in the grammars directory. See `grammars/python.scm` and `grammars/python.toml` for examples.

</details>

## Running tests

```bash
cargo test --workspace
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
grammars/                   .scm + .toml per language, registry.toml
skill/                      Claude Code skill
agent/                      Claude Code subagent definition
tests/fixtures/
  grammars-src/             vendored C sources for test .so compilation
  sample-repo/              Python fixture project
```
