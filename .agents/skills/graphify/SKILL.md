---
name: graphify
description: Optional knowledge-graph lookup and export for this repository. Use when graph-backed architecture context is useful.
---

# Graphify

Use the graph when it is faster or clearer than manually tracing relationships:

- `nodesify-graphify query "exact symbol or implementation question" --graph .`
- `nodesify-graphify map --graph .`
- `nodesify-graphify export --format html --graph .`
- `nodesify-graphify stats --graph .`

The graph is maintained automatically by the platform PostToolUse hook after edits, and by the Git post-commit/post-checkout hooks when installed. If the graph is absent or clearly stale, run `nodesify-graphify update .` (or `run .` for a first build).

## Use judgment

Graphify is strongest for exact-symbol lookup, repository maps, dependency relationships, and exports. Do not force it for natural-language discovery, simple filename searches, or questions requiring runtime behavior. The `path` and `affected` commands are specialized analyses; use them only when the node names and direction are already known.

This skill is opt-in. Ordinary file reading and search remain valid.
