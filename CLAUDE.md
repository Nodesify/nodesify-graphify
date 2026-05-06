## graphify

Rust rewrite of graphify — turns any folder into a queryable knowledge graph. Published as `@nodesify/graphify` via npm.

### Architecture

Rust workspace (12 crates) + Node.js CLI:

- `crates/graphify-core` — types (`FileType`, `GraphStats`), `GraphifyError`, SQLite schema (6 tables), path validation and sanitization
- `crates/graphify-paths` — path normalization and `.graphify` directory management
- `crates/graphify-detect` — file discovery, classification, incremental change detection via SHA-256 manifest
- `crates/graphify-extract` — tree-sitter AST extraction (20 languages), per-language configs in `src/langs/`. Extraction schema types (`Extraction`, `ExtractedNode`, `ExtractedEdge`) in `src/schema.rs`.
- `crates/graphify-build` — merge extractions into SQLite graph with deduplication
- `crates/graphify-cluster` — label propagation community detection (petgraph)
- `crates/graphify-analyze` — god nodes, surprising cross-community connections, suggested questions
- `crates/graphify-report` — markdown report generation (`graph_report.md`)
- `crates/graphify-semantic` — LLM-based semantic extraction (Claude API), enriches nodes/edges with topics and concepts
- `crates/graphify-ingest` — URL ingestion with SSRF protection, HTML-to-text conversion
- `crates/graphify-pdf` — PDF text extraction
- `crates/graphify-napi` — napi-rs bindings, pipeline orchestration (`pipeline.rs`), query engine (`query.rs`), merge/diff (`merge.rs`), HTML/GraphML export
- `packages/graphify-cli` — Node.js CLI (commander.js), thin wrapper over napi bindings

Pipeline: `detect() → extract() → enrich_with_semantics() → build() → cluster() → analyze() → report()`

The semantic enrichment step is optional — it activates when `GRAPHIFY_LLM_API_KEY` is set, calling the Claude API to extract topics, concepts, and entities.

Persistence: single `.graphify/db.sqlite` (extraction cache, file manifest, nodes/edges, pipeline runs, query history).

### CLI commands

```
nodesify-graphify run <path>                                    # Full pipeline
nodesify-graphify update <path>                                 # Incremental rebuild
nodesify-graphify watch <path> [--debounce 3000]                # File watcher (Node.js-side)
nodesify-graphify explain <node> [--graph .]                    # Node explanation + connections
nodesify-graphify query <question> [--dfs] [--depth 2] [--budget 2000] [--graph .]  # BFS/DFS traversal
nodesify-graphify path <A> <B> [--graph .]                      # Shortest path
nodesify-graphify stats [--graph .]                             # Graph statistics
nodesify-graphify export [--graph .] [--out graph.json] [--format json|html|graphml]  # Export graph
nodesify-graphify merge <pathA> <pathB> <outPath>               # Merge two graphs
nodesify-graphify diff <pathA> <pathB>                          # Compare two graphs
nodesify-graphify history [--limit 20] [--graph .]              # Show recent query history
nodesify-graphify cluster-only <path>                           # Re-cluster without re-extracting
nodesify-graphify install [--platform claude|codex|gemini|cursor|kiro|aider|copilot|trae|opencode]  # Install AI platform skill files
nodesify-graphify uninstall [--platform ...]                    # Uninstall AI platform skill files
nodesify-graphify hook install|uninstall|status                 # Git hook management
```

### Build

```bash
cargo build --release              # Rust core
cd packages/graphify-cli && npm run build  # CLI
```

### Test

```bash
cargo test  # Each crate has unit tests using in-memory SQLite and tempfile fixtures
```

### Other agent configs

- `GEMINI.md` — equivalent config for Gemini CLI (synced with this file)
- `packages/graphify-cli/skills/` — platform-specific skill files (skill.md for Claude, skill-codex.md, skill-gemini.md, skill-opencode.md)

### Knowledge graph

This project has a nodesify-graphify knowledge graph at `.graphify/`.

Rules:
- Before answering architecture or codebase questions, read `.graphify/graph_report.md` for god nodes and community structure
- For cross-module "how does X relate to Y" questions, prefer `nodesify-graphify query "<question>"`, `nodesify-graphify path "<A>" "<B>"`, or `nodesify-graphify explain "<concept>"` over grep
- After modifying code files in this session, run `nodesify-graphify update .` to keep the graph current
