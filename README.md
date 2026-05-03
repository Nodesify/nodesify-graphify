# nodesify-graphify

Turn any folder into a queryable knowledge graph. Rust core, Node.js CLI.

## Install

```bash
npm install -g @nodesify/graphify
```

Requires no Rust toolchain — ships prebuilt native binaries via napi-rs.

## Usage

```bash
nodesify-graphify run <path>                            # Full pipeline: detect → extract → build → cluster → analyze → report
nodesify-graphify update <path>                         # Incremental rebuild (only changed files)
nodesify-graphify watch <path> [--debounce 3000]        # Watch for file changes, auto-rebuild
nodesify-graphify explain <node> [--graph .]            # Explain a node and its connections
nodesify-graphify query <question> [--dfs] [--budget 2000] [--graph .]  # BFS/DFS traversal
nodesify-graphify path <A> <B> [--graph .]              # Shortest path between two concepts
nodesify-graphify stats [--graph .]                     # Node/edge/community counts
nodesify-graphify export [--graph .] [--out graph.json] [--format json|html|graphml] # Export graph
nodesify-graphify merge <pathA> <pathB> <outPath>       # Merge two graphs
nodesify-graphify diff <pathA> <pathB>                  # Compare two graphs
nodesify-graphify history [--limit 20] [--graph .]      # Show recent query history
nodesify-graphify install [--platform claude]           # Install skill files for AI coding assistants
nodesify-graphify hook install|uninstall|status         # Git hook management
```

Supported platforms for `install`: `claude`, `codex`, `gemini`, `cursor`, `copilot`, `aider`, `opencode`, `kiro`, `trae`.

Running `nodesify-graphify run .` creates `.graphify/` with:

- `db.sqlite` — the graph database
- `graph.json` — full graph export
- `graph_report.md` — report with hub nodes, communities, surprising connections

### .graphifyignore

Place a `.graphifyignore` file in your project root (gitignore syntax) to exclude files from the graph.

## Architecture

Rust workspace with 8 domain crates + Node.js CLI:

```
crates/
  graphify-core/      Types, error, SQLite schema
  graphify-detect/    File discovery, classification, incremental change detection
  graphify-extract/   Tree-sitter AST extraction (22 languages)
  graphify-build/     Merge extractions into SQLite graph
  graphify-cluster/   Label propagation community detection
  graphify-analyze/   God nodes, surprising connections, suggested questions
  graphify-report/    Markdown report generation
  graphify-napi/      napi-rs bindings, pipeline orchestration, query engine
packages/
  graphify-cli/       Node.js CLI (commander.js)
```

Pipeline: `detect() → extract() → build() → cluster() → analyze() → report()`

Each stage is a pure function in its own crate. SQLite is the persistence layer (extraction cache, file manifest, graph storage, pipeline state, query history). petgraph provides in-memory algorithms (BFS/DFS, label propagation, shortest path).

Design docs: [design spec](docs/superpowers/specs/2026-04-30-nodesify-graphify-rewrite-design.md), [implementation plan](docs/superpowers/plans/2026-04-30-nodesify-graphify-implementation.md).

## Build from source

```bash
# Build Rust core
cargo build --release

# Build Node.js CLI
cd packages/graphify-cli && npm run build
```

Requires Rust 2021 edition (Rust 1.56+) and Node.js >= 18.

## Test

```bash
cargo test  # All Rust crate unit tests
```

Each crate has unit tests using in-memory SQLite (`open_db_in_memory()`) and `tempfile` for filesystem fixtures. No integration test suite or CLI tests yet.

## Language support

Python, JavaScript, TypeScript, Rust, Go, Java, C, C++, Ruby, Swift, Scala, PHP, C#, Lua, Haskell, Elixir, Bash, Dart, Zig, CSS — via tree-sitter grammars. (Note: Kotlin support is currently disabled due to tree-sitter version incompatibility).

Each language has its own config module in `crates/graphify-extract/src/langs/`. Adding a new language means adding a new file there and registering it in `langs/mod.rs`.

## License

MIT
