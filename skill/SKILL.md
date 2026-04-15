---
name: eagraph
description: Code knowledge graph for navigating multi-repo codebases. ALWAYS use eagraph instead of multiple glob/grep/read calls when investigating code structure, callers/callees, dependencies, or file contents. One eagraph call replaces 5-15 tool calls.
---

# eagraph — Code Knowledge Graph

**Use eagraph FIRST before resorting to glob/grep/read** for any code navigation task. It is faster, cheaper, and returns structured results.

`--repo` is optional on all commands — eagraph auto-detects the repo from the current working directory.

All commands support `--json` for structured output: `eagraph --json <command> ...`

Data is always fresh — eagraph auto-refreshes stale files before every query.

## When to use

- Understanding a symbol → `eagraph --json context <name>`
- Finding callers/dependents → `eagraph --json dependents <file>`
- File structure → `eagraph --json symbols <file>`
- Tracing call chains → `eagraph --json chain <from> <to>`
- Finding symbols by name → `eagraph --json query <name>`

## Commands

### Get structural context for a symbol

```bash
eagraph --json context <symbol-name>
```

Returns the symbol's source code, all symbols it calls/imports/inherits, and all symbols that call/import/inherit it, with source snippets. Use `--depth 3` for deeper traversal.

### Get dependents of a file

```bash
eagraph --json dependents <file-path>
```

Returns every symbol in the file and what depends on each one. File path is relative to the repo root.

### List all symbols in a file

```bash
eagraph --json symbols <file-path>
```

Table of contents for a file — every function, class, method with line ranges. Use this instead of reading entire files to understand structure.

### Find shortest call path between two symbols

```bash
eagraph --json chain <from-symbol> <to-symbol>
```

### Search for symbols by name

```bash
eagraph --json query <name>
```

### Check index status

```bash
eagraph --json status
```

### Re-index a repo

```bash
eagraph index <repo-name>
eagraph index <repo-name> --force
```

### Interactive graph visualization

```bash
eagraph viz
```

Starts a local web server with an interactive force-directed graph.

## Tips

- `--repo` is auto-detected from cwd. Only specify it when working across repos.
- Use `--json` for all queries.
- Use `context` as the first step when investigating any symbol.
- Use `symbols` to understand a file's structure without reading it.
- Use `dependents` before making changes to understand impact.
- Depth 1 is usually enough. Use depth 2-3 for longer chains.

## Installation

```bash
ln -s /path/to/eagraph/skill ~/.claude/skills/eagraph
ln -s /path/to/eagraph/agent/eagraph-explorer.md ~/.claude/agents/eagraph-explorer.md
```

The agent ensures subagents use eagraph for code navigation instead of raw grep/glob/read.
