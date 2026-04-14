---
name: eagraph
description: Code knowledge graph for navigating multi-repo codebases. Use when you need to understand code structure, find callers/callees, trace dependencies, or get context around a symbol. Replaces multiple glob/grep/read calls with a single graph query.
---

# eagraph — Code Knowledge Graph

Use this skill to query the code knowledge graph instead of doing multiple file searches. One `eagraph` call replaces 5-15 glob/grep/read tool calls.

All commands support `--json` for structured output: `eagraph --json <command> ...`

## When to use

- "What calls this function?" → `eagraph --json context <name> --repo <repo>`
- "What breaks if I change this file?" → `eagraph --json dependents <file> --repo <repo>`
- "What's in this file?" → `eagraph --json symbols <file> --repo <repo>`
- "How does A reach B?" → `eagraph --json chain <from> <to> --repo <repo>`
- "Find a function by name" → `eagraph --json query <name>`

## Commands

### Get structural context for a symbol

```bash
eagraph --json context <symbol-name> --repo <repo-name> --depth 2
```

Returns the symbol's source code, all symbols it calls/imports/inherits, and all symbols that call/import/inherit it, with source snippets. Depth controls how many hops to traverse.

### Get dependents of a file

```bash
eagraph --json dependents <file-path> --repo <repo-name> --depth 1
```

Returns every symbol in the file and what depends on each one. File path is relative to the repo root.

### List all symbols in a file

```bash
eagraph --json symbols <file-path> --repo <repo-name>
```

Returns every symbol (function, class, method) in the file with kind and line range. Like a table of contents — avoids reading the whole file.

### Find shortest call path between two symbols

```bash
eagraph --json chain <from-symbol> <to-symbol> --repo <repo-name>
```

Returns the shortest sequence of call edges connecting two symbols. Returns null if no path exists.

### Search for symbols by name

```bash
eagraph --json query <name> --repo <repo-name>
```

Substring search across all symbols. Returns name, kind, file path, line range, and repo.

### Check index status

```bash
eagraph --json status
```

Returns each repo's name, branch, and symbol count.

### Re-index a repo

```bash
eagraph index <repo-name>
eagraph index <repo-name> --force  # full re-index, fresh DB
```

## Tips

- Use `--json` for all queries — structured output is easier to parse and uses fewer tokens than the human-readable format.
- Use `context` as the first step when investigating any symbol — it gives you the full picture in one call.
- Use `symbols` to understand a file's structure without reading it.
- Use `dependents` before making changes to understand impact.
- Use `chain` to trace how a request flows through the system.
- Depth 1 is usually enough. Use depth 2-3 for tracing longer chains.
- The graph only contains symbols from indexed files. External library calls won't have targets.

## Installation

```bash
ln -s /path/to/eagraph/skill ~/.claude/skills/eagraph
```
