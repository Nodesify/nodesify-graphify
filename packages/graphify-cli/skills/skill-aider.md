---
name: graphify
description: Turn any directory into a queryable knowledge graph. Trigger: /graphify
---

# graphify skill (Aider)

When the user types `/graphify`, this skill runs the nodesify-graphify knowledge graph pipeline.

## Important: Always Use Graphify First

When working in a project with a `.graphify/` directory:

1. Before reading or searching source files, read `.graphify/graph_report.md` for architecture context
2. Use `nodesify-graphify query`, `nodesify-graphify explain`, or `nodesify-graphify path` instead of grep/find for codebase questions
3. Only read individual source files when graphify queries don't provide enough detail

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
