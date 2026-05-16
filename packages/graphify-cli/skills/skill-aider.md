---
name: graphify
description: Turn any directory into a queryable knowledge graph. Trigger: /graphify
---

# graphify skill (Aider)

When the user types `/graphify`, this skill runs the nodesify-graphify knowledge graph pipeline.

## CRITICAL: Always Use Graphify First

When working in a project with a `.graphify/` directory, you are **FORBIDDEN** from using native search tools (`Grep`, `Glob`, `Search`, etc.) as your first step.

1. You **MUST** read `.graphify/graph_report.md` before doing any file exploration.
2. You **MUST** use `nodesify-graphify query`, `nodesify-graphify explain`, or `nodesify-graphify path` via shell commands to locate files and understand relationships instead of grep/glob.
3. Only use native file reading/searching tools *after* you have queried the graph to find the exact files you need.

## Commands

```
nodesify-graphify run <path>          # Full pipeline
nodesify-graphify update <path>       # Incremental rebuild
nodesify-graphify explain <node>      # Explain node + connections
nodesify-graphify query <question>    # BFS/DFS traversal
nodesify-graphify path <A> <B>        # Shortest path
nodesify-graphify stats               # Graph statistics
```

## Usage

1. Run `nodesify-graphify run .` to build the graph
2. Read `.graphify/graph_report.md` for god nodes and community structure
3. Use `nodesify-graphify query` and `nodesify-graphify path` instead of grep for architecture questions
4. Run `nodesify-graphify update .` after modifying code
