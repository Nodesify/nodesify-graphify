## graphify

Rust rewrite of graphify — turns any folder into a queryable knowledge graph. Published as `@nodesify/graphify` via npm.

### Architecture

Rust workspace (14 crates) + Node.js CLI:

- `crates/graphify-core` — types (`FileType`, `GraphStats`), `GraphifyError`, SQLite schema + migrations, path validation, sanitization, sensitive-path denylist
- `crates/graphify-paths` — path normalization and `.graphify` directory management
- `crates/graphify-detect` — file discovery, classification, incremental change detection via SHA-256 manifest
- `crates/graphify-extract` — tree-sitter AST extraction (21 languages), per-language configs in `src/langs/`. Extraction schema types (`Extraction`, `ExtractedNode`, `ExtractedEdge`) in `src/schema.rs`.
- `crates/graphify-build` — merge extractions into SQLite graph; entity dedup (MinHash/LSH blocking + Jaro-Winkler verify) in `dedup.rs`
- `crates/graphify-cluster` — deterministic label propagation community detection (petgraph), hub labels, cohesion, modularity
- `crates/graphify-analyze` — god nodes (call stubs excluded), ranked surprising cross-community connections, blast radius (`affected.rs`, reverse reachability)
- `crates/graphify-query` — query engine: BFS/DFS (optionally directed), shortest path, explain; token-based node scoring; per-path graph cache
- `crates/graphify-mcp` — MCP stdio server exposing the graph to AI agents
- `crates/graphify-report` — markdown report generation (`graph_report.md`)
- `crates/graphify-semantic` — LLM semantic extraction, multi-backend (Claude / OpenAI-compatible / Gemini) with vision, chunking, and output validation
- `crates/graphify-ingest` — URL ingestion (arXiv/tweet/webpage/image) with SSRF protection
- `crates/graphify-pdf` — PDF text extraction
- `crates/graphify-napi` — napi-rs bindings, pipeline orchestration (`pipeline.rs`), merge/diff (`merge.rs`), JSON/HTML/GraphML/tree export
- `packages/graphify-cli` — Node.js CLI (commander.js), thin wrapper over napi bindings

Pipeline: `detect() → extract() → enrich_with_semantics() → build() → dedup_nodes() → cluster() → analyze() → report()`

The semantic enrichment step is optional — it activates when a backend is configured (`GRAPHIFY_LLM_BACKEND`, or `GRAPHIFY_LLM_API_KEY` / `OPENAI_API_KEY` / `GEMINI_API_KEY`), extracting topics, concepts, and entities (including from images via vision). Cache misses are extracted concurrently by a worker pool; `GRAPHIFY_LLM_CONCURRENCY` (1–8, default 4) controls the worker count.

Persistence: single `.graphify/db.sqlite` (extraction cache, file manifest, nodes/edges, pipeline runs, query history).

### CLI commands

```bash
nodesify-graphify run <path>                                    # Full pipeline
nodesify-graphify update <path>                                 # Incremental rebuild
nodesify-graphify watch <path> [--debounce 3000]                # File watcher
nodesify-graphify explain <node> [--graph .]                    # Node explanation + connections
nodesify-graphify query <question> [--dfs] [--depth 2] [--budget 2000] [--directed] [--graph .]  # BFS/DFS traversal
nodesify-graphify path <A> <B> [--directed] [--graph .]                      # Shortest path
nodesify-graphify affected <node> [--depth 2] [--relation R] [--graph .]  # Blast radius - what breaks if you change this node
nodesify-graphify add <url> [--author] [--contributor]     # Fetch arXiv/tweet/webpage/image/PDF into ./raw + update graph
nodesify-graphify mcp [--graph .]                             # Run MCP stdio server - query the graph from any AI agent
nodesify-graphify tree [--out tree.html] [--max-children 40] # Collapsible filesystem tree of all symbols (HTML)
nodesify-graphify prs [20] [--conflicts] [--graph .]         # Map open PRs onto the graph - impact + merge-order risk
nodesify-graphify stats [--graph .]                             # Graph statistics
nodesify-graphify export [--graph .] [--out graph.json] [--format json|html|graphml] # Export graph
nodesify-graphify merge <pathA> <pathB> <outPath>               # Merge two graphs
nodesify-graphify diff <pathA> <pathB>                          # Compare two graphs
nodesify-graphify history [--limit 20] [--graph .]              # Show recent query history
nodesify-graphify install [--platform claude|gemini|...]        # Install AI platform skill files
nodesify-graphify hook install|uninstall|status                 # Git hook management
```

### Supported languages

Python, JavaScript, TypeScript, Rust, Go, Java, C, C++, Ruby, Swift, Kotlin, Scala, PHP, C#, Lua, Haskell, Elixir, Bash, Dart, Zig, CSS.

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
- `packages/graphify-cli/skills/` — platform-specific skill files (skill.md for Codex, skill-codex.md, skill-gemini.md, skill-opencode.md)

### Knowledge graph

This project has a nodesify-graphify knowledge graph at `.graphify/`.

CRITICAL RULES:
- You are **FORBIDDEN** from using native search tools (`Grep`, `Glob`, `Search`, etc.) as your first step for discovery.
- You **MUST** read `.graphify/graph_report.md` before doing any file exploration.
- You **MUST** use `nodesify-graphify query "<question>"`, `nodesify-graphify path "<A>" "<B>"`, or `nodesify-graphify explain "<concept>"` via shell commands to locate files, components, and understand cross-module relationships.
- Only use native file reading/searching tools *after* you have queried the graph to find the exact files you need.
- After modifying code files in this session, run `nodesify-graphify update .` to keep the graph current.
