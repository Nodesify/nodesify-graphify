## graphify

Rust rewrite of graphify — turns any folder into a queryable knowledge graph. Published as `@nodesify/graphify` via npm.

### Architecture

Rust workspace (8 crates) + Node.js CLI:

- `crates/graphify-core` — shared types (`FileType`, `Relation`, `Confidence`), `GraphifyError`, SQLite schema
- `crates/graphify-detect` — file discovery, classification, incremental change detection via SHA-256 manifest
- `crates/graphify-extract` — tree-sitter AST extraction (21 languages), per-language configs in `src/langs/`
- `crates/graphify-build` — merge extractions into SQLite graph with deduplication
- `crates/graphify-cluster` — label propagation community detection (petgraph)
- `crates/graphify-analyze` — god nodes, surprising cross-community connections, suggested questions
- `crates/graphify-report` — markdown report generation (`graph_report.md`)
- `crates/graphify-napi` — napi-rs bindings, pipeline orchestration (`pipeline.rs`), query engine (`query.rs` — BFS/DFS, shortest path, explain)
- `packages/graphify-cli` — Node.js CLI (commander.js), thin wrapper over napi bindings

Pipeline: `detect() → extract() → build() → cluster() → analyze() → report()`

Persistence: single `.graphify/db.sqlite` (extraction cache, file manifest, nodes/edges, pipeline runs, query history).

### CLI commands

```bash
nodesify-graphify run <path>                                    # Full pipeline
nodesify-graphify update <path>                                 # Incremental rebuild
nodesify-graphify watch <path> [--debounce 3000]                # File watcher
nodesify-graphify explain <node> [--graph .]                    # Node explanation + connections
nodesify-graphify query <question> [--dfs] [--budget 2000] [--graph .]  # BFS/DFS traversal
nodesify-graphify path <A> <B> [--graph .]                      # Shortest path
nodesify-graphify stats [--graph .]                             # Graph statistics
nodesify-graphify export [--graph .] [--out graph.json] [--format json|html|graphml] # Export graph
nodesify-graphify merge <pathA> <pathB> <outPath>               # Merge two graphs
nodesify-graphify diff <pathA> <pathB>                          # Compare two graphs
nodesify-graphify history [--limit 20] [--graph .]              # Show recent query history
nodesify-graphify install [--platform claude|gemini|...]        # Install AI platform skill files
nodesify-graphify hook install|uninstall|status                 # Git hook management
```

### Supported languages

Python, JavaScript, TypeScript, Rust, Go, Java, C, C++, Ruby, Swift, Scala, PHP, C#, Lua, Haskell, Elixir, Bash, Dart, Zig, CSS. (Note: Kotlin support is currently disabled).

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
