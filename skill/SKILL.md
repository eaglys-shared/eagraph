---
name: eagraph
description: Code knowledge graph for navigating multi-repo codebases. Use when you need to understand code structure, find callers/callees, trace dependencies, or get context around a symbol. Replaces multiple glob/grep/read calls with a single graph query.
---

# eagraph — Code Knowledge Graph

Use this skill to query the code knowledge graph instead of doing multiple file searches. One `eagraph` call replaces 5-15 glob/grep/read tool calls.

## When to use

- "What calls this function?" → `eagraph context <name> --repo <repo>`
- "What breaks if I change this file?" → `eagraph dependents <file> --repo <repo>`
- "Where is X defined and what does it depend on?" → `eagraph context <name> --repo <repo>`
- "Show me the class hierarchy" → `eagraph context <class> --repo <repo> --depth 3`
- "Find a function by name" → `eagraph query <name>`

## Commands

### Get structural context for a symbol

```bash
eagraph context <symbol-name> --repo <repo-name> --depth 2
```

Returns the symbol's source code, all symbols it calls/imports/inherits, and all symbols that call/import/inherit it, with source snippets. Depth controls how many hops to traverse.

### Get dependents of a file

```bash
eagraph dependents <file-path> --repo <repo-name> --depth 1
```

Returns every symbol in the file and what depends on each one. File path is relative to the repo root.

### Search for symbols by name

```bash
eagraph query <name> --repo <repo-name>
```

Substring search across all symbols. Returns name, kind, file path, and line range.

### Check index status

```bash
eagraph status
```

### Re-index a repo

```bash
eagraph index <repo-name>
eagraph index <repo-name> --force  # full re-index, fresh DB
```

## Configured repos

Check available repos with `eagraph status`. The config is at the path shown by `eagraph config`.

## Tips

- Use `context` as the first step when investigating any symbol — it gives you the full picture in one call.
- Use `dependents` before making changes to understand impact.
- Depth 1 is usually enough for understanding immediate relationships. Use depth 2-3 for tracing longer chains.
- The graph only contains symbols from indexed files. External library calls won't have targets.

## Installation

To install this skill for Claude Code, copy or symlink to `~/.claude/skills/eagraph/`:

```bash
ln -s /path/to/eagraph/skill ~/.claude/skills/eagraph
```
