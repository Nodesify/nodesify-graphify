# Graphify Architecture Reference

nodesify-graphify turns source code into a queryable knowledge graph. It uses AST-based extraction via tree-sitter for deterministic, fast analysis, stored in a SQLite database.

## Overview

The project is structured as a Rust workspace with 8 domain-specific crates and a Node.js CLI.

**Language**: Rust 2021
**Build system**: Cargo + npm
**Core dependencies**: `rusqlite` (persistence), `tree-sitter` (AST parsing), `petgraph` (graph algorithms), `napi-rs` (Node.js bindings)

## Pipeline

```
detect() → extract() → build() → cluster() → analyze() → report()
```

The pipeline is orchestrated in `crates/graphify-napi/src/pipeline.rs`.

1.  **detect()** (`graphify-detect`): Discovers files, classifies them (Code, Document, etc.), and uses a SHA-256 manifest to identify changed files since the last run.
2.  **extract()** (`graphify-extract`): Performs AST-based extraction using tree-sitter. Supports 21 languages with per-language configurations in `src/langs/`.
3.  **build()** (`graphify-build`): Merges extracted nodes and edges into the SQLite graph database, handles deduplication and cross-file reference resolution.
4.  **cluster()** (`graphify-cluster`): Performs community detection using the label propagation algorithm (via `petgraph`) and updates the `community` attribute on nodes.
5.  **analyze()** (`graphify-analyze`): Analyzes the graph to find "god nodes" (high degree), surprising cross-community connections, and generates suggested questions.
6.  **report()** (`graphify-report`): Generates a plain-language `graph_report.md` summarizing the graph's structure and insights.

## Crate Responsibilities

| Crate | Responsibility |
| :--- | :--- |
| `graphify-core` | Shared types (`FileType`, `Relation`, `Confidence`), `GraphifyError`, and SQLite schema definitions. |
| `graphify-detect` | File system scanning, `.graphifyignore` support, and incremental change detection via SHA-256 hashes. |
| `graphify-extract` | Tree-sitter AST traversal logic. Each language defines its own extraction rules (nodes, edges, docstrings). |
| `graphify-build` | Persistent graph assembly. Handles the SQL-heavy work of merging extractions into the database. |
| `graphify-cluster` | Community detection and graph topology analysis using `petgraph`. |
| `graphify-analyze` | Heuristic-based graph analysis for discovering architectural patterns and anomalies. |
| `graphify-report` | Markdown generation for the final user-facing report. |
| `graphify-napi` | The bridge between Rust and Node.js. Includes the query engine (BFS/DFS, shortest path) and CLI command handlers. |
| `graphify-cli` | The Node.js-based user interface, responsible for argument parsing and installing AI skills. |

## Data Models

### SQLite Schema

The graph is stored in `.graphify/db.sqlite` with the following key tables:

*   `nodes`: `id`, `label`, `file_type`, `source_file`, `source_line`, `docstring`, `community`.
*   `edges`: `source`, `target`, `relation`, `confidence`, `confidence_score`, `source_file`, `source_line`.
*   `file_manifest`: `path`, `hash`, `last_extracted_at`. Used for incremental updates.
*   `query_history`: `question`, `answer`, `queried_at`.

### Relationship Types

*   `Calls`: Function or method invocation.
*   `Imports`: Module or file level dependency.
*   `Uses`: Variable or type usage.
*   `Defines`: Containment (e.g., class defines a method).
*   `Inherits`: Class inheritance or interface implementation.

## Persistence & Performance

*   **SQLite**: Chosen for its zero-config nature and robust ACID properties, making it perfect for local analysis.
*   **Incremental Rebuilds**: The system only re-extracts files that have changed, drastically reducing analysis time for large projects.
*   **napi-rs**: Provides near-native performance for the CLI while maintaining the ease of use of an npm package.

## Language Support

Extraction rules are defined in `crates/graphify-extract/src/langs/`. Each language module provides a `LanguageConfig` specifying which AST nodes represent classes, functions, and relationships.

Currently supported: Python, JS, TS, Rust, Go, Java, C, C++, Ruby, Swift, Scala, PHP, C#, Lua, Haskell, Elixir, Bash, Dart, Zig, CSS. (Kotlin is currently disabled).
