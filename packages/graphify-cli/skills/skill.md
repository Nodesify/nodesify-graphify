---
name: graphify
description: Turn any directory into a queryable knowledge graph. Trigger: /graphify
---

# /graphify

Turn any directory of source code into a queryable knowledge graph with community detection, hub node analysis, and a plain-language graph report. Uses AST-based extraction via tree-sitter for deterministic, fast analysis.

## What You Must Do When Invoked

If no path was given, use `.` (current directory). Do not ask the user for a path.

Follow these steps in order. Do not skip steps.

### Step 1 - Check graph state and build if needed

```bash
node -e "const fs=require('fs');const p='.graphify/graph.json';if(!fs.existsSync(p)){console.log('missing');process.exit(0)}const age=Math.round((Date.now()-fs.statSync(p).mtimeMs)/60000);console.log(age>30?'stale':'fresh')"
```

Act on the result:
- `missing`: Run `nodesify-graphify run .` — this builds the full graph from scratch. Wait for it to complete.
- `stale`: Run `nodesify-graphify update .` — incremental rebuild of changed files. Wait for it to complete.
- `fresh`: Skip to Step 2.

After running `run` or `update`, verify the output shows `Nodes added` > 0 or `Communities` > 0. If the graph is empty (0 nodes), tell the user: "No supported files found. graphify supports Python, JS, TS, Rust, Go, Java, C, C++, Ruby, Swift, Scala, PHP, C#, Lua, Haskell, Elixir, Bash, Dart, Zig, CSS, Markdown, plain text, and PDF."

### Step 2 - Read the graph report

Read `.graphify/graph_report.md`. Present a clean summary:

```
Knowledge graph built: X nodes, Y edges, Z communities

Hub nodes (most connected):
  1. NodeName — N connections (community C)
  2. ...

Surprising connections:
  - SourceNode → TargetNode [cross-community edge]

Suggested questions:
  - "Question 1?"
  - "Question 2?"
```

Do NOT dump the raw report. Summarize it.

### Step 3 - Ready for queries

Tell the user the graph is ready and they can ask questions. Example queries they might try:
- `nodesify-graphify query "authentication flow"`
- `nodesify-graphify path "AuthService" "UserModel"`
- `nodesify-graphify explain "validate_token"`

## Enforcement Rules

When working in a project with a `.graphify/` directory, these rules are MANDATORY:

### FORBIDDEN

Do NOT use `Grep`, `Glob`, or file browsing as your first step for architecture or codebase questions. This includes:
- Searching for "where is X defined"
- Looking for "how does X connect to Y"
- Exploring "what files are in this project"
- Finding "all files that reference X"

### REQUIRED

Instead, use these graphify commands:

| User asks | Use this command |
|-----------|-----------------|
| "Where is X implemented?" | `nodesify-graphify query "X"` |
| "How does X connect to Y?" | `nodesify-graphify path "X" "Y"` |
| "What does X do?" | `nodesify-graphify explain "X"` |
| "What is the architecture?" | Read `.graphify/graph_report.md` |
| "Find all references to X" | `nodesify-graphify query "X" --depth 3` |
| "What community is X in?" | `nodesify-graphify explain "X"` |

Only use native file reading AFTER the graph has identified the exact files you need.

### When native tools ARE appropriate

- Editing a specific file (after graph identified which file)
- Reading a file the user explicitly named
- Running tests or build commands
- Git operations

## Command Reference

### `nodesify-graphify run <path>`

Full pipeline: detect → extract → build → cluster → analyze → report.

Creates `.graphify/` with `db.sqlite`, `graph.json`, `graph_report.md`.

### `nodesify-graphify update <path>`

Incremental rebuild — only re-extracts files that changed (SHA-256 detection).

Much faster than `run` for existing projects.

### `nodesify-graphify query <question> [options]`

BFS (default) or DFS graph traversal from nodes matching your question.

```
nodesify-graphify query "authentication"              # BFS, depth 2, budget 2000
nodesify-graphify query "database" --dfs --depth 3    # DFS, deeper
nodesify-graphify query "error handling" --budget 3000 # more output
```

Options:
- `--dfs` — depth-first search (traces specific paths)
- `--depth <n>` — traversal depth (default: 2)
- `--budget <n>` — token budget for output (default: 2000)
- `--graph <path>` — project root (default: `.`)

### `nodesify-graphify explain <node> [options]`

Show a node's details and all its connections.

```
nodesify-graphify explain "UserService"
```

### `nodesify-graphify path <A> <B> [options]`

Find shortest path between two concepts.

```
nodesify-graphify path "AuthService" "Database"
```

### `nodesify-graphify stats [options]`

Quick graph health check: node count, edge count, communities, files.

### `nodesify-graphify status [options]`

Graph staleness check: fresh/stale/very_stale with age in minutes.

### `nodesify-graphify export [options]`

Export graph to JSON, HTML, or GraphML.

```
nodesify-graphify export --format html --out graph.html
nodesify-graphify export --format graphml --out graph.graphml
```

## Post-Edit Protocol

After modifying code files in a session with an active graph:

```bash
nodesify-graphify update .
```

Or start a watcher at session beginning:

```bash
nodesify-graphify watch . --debounce 3000
```

This keeps the graph current so subsequent queries reflect your changes.

## Troubleshooting

**"Graph is empty (0 nodes)"**
- Run `nodesify-graphify run .` to build from scratch
- Check that the directory has supported file types

**"Query returns no results"**
- Try different search terms (partial matches work)
- Use broader terms: "auth" instead of "authenticateUserWithOAuth"
- Check `nodesify-graphify stats` to verify graph has data

**"Graph seems stale"**
- Run `nodesify-graphify update .` for incremental refresh
- Or `nodesify-graphify run .` for full rebuild

**"Status says stale"**
- The `graph.json` was built more than 30 minutes ago
- Run `nodesify-graphify update .` before querying
