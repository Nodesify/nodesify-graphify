## graphify

Rust rewrite of graphify — turns any folder into a queryable knowledge graph. Published as `@nodesify/graphify` via npm.

### Architecture

Rust workspace (8 crates) + Node.js CLI:

- `crates/graphify-core` — types (`FileType`, `Relation`, `Confidence`), `GraphifyError`, SQLite schema
- `crates/graphify-detect` — file discovery, classification, incremental change detection via SHA-256 manifest
- `crates/graphify-extract` — tree-sitter AST extraction (Python, JS, TS, Rust, Go, Java, C, C++), per-language configs in `src/langs/`
- `crates/graphify-build` — merge extractions into SQLite graph with deduplication
- `crates/graphify-cluster` — label propagation community detection (petgraph)
- `crates/graphify-analyze` — god nodes, surprising cross-community connections, suggested questions
- `crates/graphify-report` — markdown report generation (`graph_report.md`)
- `crates/graphify-napi` — napi-rs bindings, pipeline orchestration (`pipeline.rs`), query engine (`query.rs`)
- `packages/graphify-cli` — Node.js CLI (commander.js), thin wrapper over napi bindings

Pipeline: `detect() → extract() → build() → cluster() → analyze() → report()`

Persistence: single `.graphify/db.sqlite` (extraction cache, file manifest, nodes/edges, pipeline runs, query history).

### Build

```bash
cargo build --release              # Rust core
cd packages/graphify-cli && npm run build  # CLI
```

### Test

```bash
cargo test  # Each crate has unit tests using in-memory SQLite and tempfile fixtures
```

### Knowledge graph

This project has a nodesify-graphify knowledge graph at `.graphify/`.

Rules:
- Before answering architecture or codebase questions, read `.graphify/graph_report.md` for god nodes and community structure
- For cross-module "how does X relate to Y" questions, prefer `nodesify-graphify query "<question>"`, `nodesify-graphify path "<A>" "<B>"`, or `nodesify-graphify explain "<concept>"` over grep
- After modifying code files in this session, run `nodesify-graphify update .` to keep the graph current
