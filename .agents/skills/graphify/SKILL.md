---
name: graphify
description: Optional knowledge-graph lookup and export for this repository. Use when graph-backed architecture context is useful.
---

# Graphify

Use the graph when it is faster or clearer than manually tracing relationships:

- `nodesify-graphify query "exact symbol or implementation question" --graph .`
- `nodesify-graphify map --graph .`
- `nodesify-graphify wiki --graph .` — markdown wiki under `.graphify/wiki/`; read `index.md` for a navigable overview
- `nodesify-graphify export --format html --graph .`
- `nodesify-graphify stats --graph .`

The graph is maintained automatically by the platform PostToolUse hook after edits, and by the Git post-commit/post-checkout hooks when installed. If the graph is absent or clearly stale, run `nodesify-graphify update .` (or `run .` for a first build).

## Use judgment

Graphify is strongest for exact-symbol lookup, repository maps, dependency relationships, and exports. Do not force it for natural-language discovery, simple filename searches, or questions requiring runtime behavior. The `path` and `affected` commands are specialized analyses; use them only when the node names and direction are already known.

The graph models entities and relationships, not expression semantics: predicate-level bugs ("is this window check off-by-one?") are invisible to it. For surgical logic, grep and file reads win; use the graph to pick the files, then read them.

This skill is opt-in. Ordinary file reading and search remain valid.

## Provenance & reference nodes (v0.6.1+)

- `query` output anchors every `NODE` line with `src=path:line` and every `EDGE` line with `@path:line`; `explain` prints `File: path:line` for the node and each neighbor. Jump straight from a hit to the source.
- Identifier-shaped string literals (env vars like `PLANE_URL`, snake_case keys, dotted/kebab/slash chains such as `harness/hr-101-fix-redis-leak`) are indexed as global `reference` nodes with `references` edges, so "where is this config key / status value used?" is a graph query. Query output ends with `# graph built at <timestamp>` for freshness judgment.
