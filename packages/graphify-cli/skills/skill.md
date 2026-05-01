---
name: graphify
description: Turn any directory into a queryable knowledge graph. Trigger: /graphify
---

# graphify skill

When the user types `/graphify`, this skill runs the nodesify-graphify knowledge graph pipeline.

## Overview

nodesify-graphify turns source code into a queryable knowledge graph stored in `.graphify/`. It uses AST-based extraction via tree-sitter for deterministic, fast analysis.

## Commands

All commands use `nodesify-graphify`. Most accept `--graph <path>` to specify the project root (default: `.`).

```
nodesify-graphify run <path>                                    # Full pipeline: detect -> extract -> build -> cluster -> analyze -> report
nodesify-graphify update <path>                                 # Incremental AST-only rebuild (detects changed files via SHA-256 manifest)
nodesify-graphify watch <path> [--debounce 3000]                # Watch for file changes, auto-rebuild
nodesify-graphify explain <node> [--graph .]                    # Explain a node and list its connections
nodesify-graphify query <question> [--dfs] [--budget 2000] [--graph .]  # BFS (default) or DFS traversal from matching nodes
nodesify-graphify path <A> <B> [--graph .]                      # Shortest path between two concepts
nodesify-graphify stats [--graph .]                             # Show node/edge/community counts
nodesify-graphify export [--graph .] [--out graph.json]         # Export graph to JSON
nodesify-graphify install [--platform claude]                   # Install skill files for AI platforms
nodesify-graphify hook install                                  # Install git post-commit/post-checkout hooks
```

Supported platforms for `install`: `claude`, `codex`, `gemini`, `cursor`, `copilot`, `aider`, `opencode`, `kiro`, `trae`.

## Step 1: Run the Pipeline

```bash
nodesify-graphify run .
```

This creates `.graphify/` with:
- `db.sqlite` — the graph database
- `graph.json` — full graph export
- `graph_report.md` — plain-language report with god nodes, communities, surprising connections

## Step 2: Read the Report

Read `.graphify/graph_report.md` to understand:
- God nodes (most-connected entities)
- Community structure (clusters of related code)
- Surprising connections (cross-community edges)
- Suggested questions for further exploration

## Step 3: Query the Graph

Use query commands instead of grep for architecture questions:

```bash
# BFS (default) traversal matching "authentication"
nodesify-graphify query "authentication flow"

# DFS traversal with custom budget
nodesify-graphify query "database connection" --dfs --budget 3000

# Shortest path between two concepts
nodesify-graphify path "AuthService" "UserModel"

# Explain a specific node
nodesify-graphify explain "validate_token"
```

## Step 4: Keep the Graph Current

After modifying code:
```bash
nodesify-graphify update .
```

Or start a watcher:
```bash
nodesify-graphify watch .
```

## Excluding Files

Place a `.graphifyignore` file in the project root (gitignore syntax) to exclude files from the graph.

## Integration

To install AI agent hooks:
```bash
nodesify-graphify install --platform claude
nodesify-graphify install --platform codex
nodesify-graphify install --platform gemini
nodesify-graphify hook install
```
