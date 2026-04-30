# Nodesify Graphify — Rust Rewrite Design

**Date**: 2026-04-30
**Status**: Approved
**Source**: Rearchitecture of `C:\Nodesify\graphify` (Python v0.5.6)

---

## Summary

Rewrite graphify in Rust, published via npm as `@nodesify/graphify`. Core pipeline first (detect, extract, build, cluster, analyze, report, export), expand to MCP server, skill system, watch mode, video transcription in later releases.

---

## Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Language | Rust | tree-sitter is Rust-native, zero-copy AST parsing |
| Distribution | napi-rs + npm | Single `npm install`, prebuilt cross-platform binaries, no Rust toolchain needed for users |
| CLI | Node.js wrapper | commander/yargs calling Rust core via napi-rs |
| Graph algorithms | petgraph | In-memory BFS/DFS, label propagation, centrality, shortest path |
| Persistence | SQLite | Single `.graphify/db.sqlite` — extraction cache, file manifest, graph storage, pipeline state, query history |
| Community detection | Label propagation | Simple, fast, no extra deps. Leiden later if needed. |
| Languages v1 | Python, JS, TS, Rust, Go, Java, C, C++ | Top 8 by usage. Tree-sitter grammars are modular, adding more is easy. |
| Export v1 | JSON | Foundation for all other export formats (HTML, GraphML, Obsidian later). |
| Crate structure | Rust workspace, domain crates | Each pipeline stage is its own crate. Compiles independently, testable in isolation, avoids the Python monolith problem. |

---

## Architecture

### Workspace Structure

```
nodesify-graphify/
├── Cargo.toml                    # workspace root
├── package.json                  # npm workspace root
├── crates/
│   ├── graphify-core/            # types, SQLite layer, pipeline orchestration
│   ├── graphify-detect/          # file discovery + classification
│   ├── graphify-extract/         # tree-sitter AST extraction
│   ├── graphify-build/           # merge extractions into graph
│   ├── graphify-cluster/         # community detection (label propagation)
│   ├── graphify-analyze/         # god nodes, surprises, questions
│   ├── graphify-report/          # markdown report generation
│   └── graphify-napi/            # napi-rs bindings
├── packages/
│   └── graphify-cli/             # Node.js CLI package
├── tests/
│   └── fixtures/                 # sample files in 8 languages
└── .github/
    └── workflows/
        └── ci.yml
```

### Pipeline

```
detect() → extract() → build() → cluster() → analyze() → report() → export()
```

Each stage is a pure function in its own crate. No shared state. Side effects only through SQLite connection passed as parameter.

### Data Flow

```
  Filesystem
      │
      ▼
  detect() ──────────────────────► DetectResult { new, changed, unchanged, removed }
      │                                    │
      │  (only new + changed files)        │
      ▼                                    ▼
  extract() ──── SQLite cache check ───► Vec<Extraction>
      │                                    │
      ▼                                    ▼
  build() ───── SQLite upsert ──────────► BuildResult
      │                                    │
      ▼                                    ▼
  cluster() ──── SQLite → petgraph ─────► ClusterResult
      │           → label propagation      │
      │           → write communities       │
      ▼                                    ▼
  analyze() ──── SQLite → petgraph ─────► AnalysisResult
      │           → degree centrality       │
      │           → cross-community edges   │
      ▼                                    ▼
  report() ───── read SQLite ───────────► String (markdown)
      │                                    │
      ▼                                    ▼
  export() ───── read SQLite ───────────► graph.json
```

---

## Crate Specifications

### graphify-core

Shared types and SQLite management. No business logic.

**Types:**

```rust
pub struct Node {
    pub id: String,              // "file.py::ClassName::method_name"
    pub label: String,
    pub file_type: FileType,
    pub source_file: PathBuf,
    pub source_location: Option<SourceLocation>,
    pub docstring: Option<String>,
    pub community: Option<u32>,
}

pub struct Edge {
    pub source: String,
    pub target: String,
    pub relation: Relation,
    pub confidence: Confidence,
    pub confidence_score: Option<f64>,
    pub source_file: PathBuf,
}

pub enum FileType { Code, Document, Paper, Image, Video }

pub enum Relation {
    Calls, Imports, Uses, Defines, Contains,
    Inherits, References,
    Rationale { tag: String },
}

pub enum Confidence { Extracted, Inferred, Ambiguous }

pub struct SourceLocation { pub line: u32, pub column: Option<u32> }
```

**SQLite schema (6 tables):**

```sql
-- Extraction cache: skip re-parsing unchanged files
CREATE TABLE extraction_cache (
    file_path TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    language TEXT NOT NULL,
    nodes TEXT NOT NULL,
    edges TEXT NOT NULL,
    extracted_at TEXT NOT NULL
);

-- File manifest: incremental detection
CREATE TABLE file_manifest (
    file_path TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    file_type TEXT NOT NULL,
    language TEXT,
    last_seen_at TEXT NOT NULL,
    size_bytes INTEGER NOT NULL
);

-- Graph nodes
CREATE TABLE nodes (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    file_type TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_line INTEGER,
    docstring TEXT,
    community INTEGER,
    degree_centrality REAL
);
CREATE INDEX idx_nodes_file ON nodes(source_file);
CREATE INDEX idx_nodes_community ON nodes(community);

-- Graph edges
CREATE TABLE edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL REFERENCES nodes(id),
    target TEXT NOT NULL REFERENCES nodes(id),
    relation TEXT NOT NULL,
    confidence TEXT NOT NULL,
    confidence_score REAL,
    source_file TEXT NOT NULL
);
CREATE INDEX idx_edges_source ON edges(source);
CREATE INDEX idx_edges_target ON edges(target);

-- Pipeline run tracking
CREATE TABLE pipeline_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    status TEXT NOT NULL,
    files_processed INTEGER,
    nodes_added INTEGER,
    edges_added INTEGER
);

-- Query history
CREATE TABLE query_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    question TEXT NOT NULL,
    answer TEXT,
    path_taken TEXT,
    queried_at TEXT NOT NULL
);
```

**Pipeline orchestration:**

```rust
pub fn run_pipeline(root: &Path, db: &Connection) -> PipelineResult {
    let detected = detect::detect(root, db)?;
    let files_to_process: Vec<&PathBuf> = detected.new.iter().chain(detected.changed.iter()).collect();
    let extractions = extract::extract(&files_to_process, db)?;
    let build_result = build::build(&extractions, db)?;
    let cluster_result = cluster::cluster(db)?;
    let analysis = analyze::analyze(db)?;
    let report = report::generate_report(db, &analysis)?;
    PipelineResult { build_result, cluster_result, analysis, report }
}
```

**Output directory:**

```
.graphify/
├── db.sqlite          # single source of truth
├── graph_report.md    # generated report
└── graph.json         # exported output
```

---

### graphify-detect

File discovery + classification. Incremental via SQLite file_manifest.

```rust
pub fn detect(root: &Path, db: &Connection) -> DetectResult

pub struct DetectResult {
    pub new: Vec<FileEntry>,
    pub changed: Vec<FileEntry>,
    pub unchanged: Vec<FileEntry>,
    pub removed: Vec<FileEntry>,
}

pub struct FileEntry {
    pub path: PathBuf,
    pub file_type: FileType,
    pub language: Option<String>,
    pub content_hash: String,
    pub size_bytes: u64,
}
```

- Walks filesystem recursively
- Classifies by extension: `.py` → Code/Python, `.md` → Document, `.pdf` → Paper, etc.
- Checks `file_manifest` table for each file — only returns new/changed files
- Marks files not seen in current walk as removed

---

### graphify-extract

Tree-sitter AST extraction. Each language in its own file.

```
crates/graphify-extract/src/
├── lib.rs           # public API
├── engine.rs        # tree-sitter orchestration, two-pass logic
├── schema.rs        # Extraction, ExtractedNode, ExtractedEdge
└── langs/
    ├── mod.rs        # registry
    ├── python.rs
    ├── javascript.rs
    ├── typescript.rs
    ├── rust.rs
    ├── go.rs
    ├── java.rs
    ├── c.rs
    └── config.rs     # LanguageConfig struct
```

```rust
pub struct LanguageConfig {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub class_types: &'static [&'static str],
    pub function_types: &'static [&'static str],
    pub import_types: &'static [&'static str],
    pub call_type: &'static str,
    pub name_field: &'static str,
    pub body_field: Option<&'static str>,
}

pub struct Extraction {
    pub file_path: PathBuf,
    pub language: String,
    pub nodes: Vec<ExtractedNode>,
    pub edges: Vec<ExtractedEdge>,
}

pub fn extract(files: &[&PathBuf], db: &Connection) -> Vec<Extraction>
```

**Two-pass approach:**
1. **Structural pass**: Walk AST for classes, functions, imports, docstrings, rationale comments. Produce nodes + structural edges (defines, contains, imports).
2. **Call-graph pass**: Walk AST for function calls. Resolve cross-file references. Produce INFERRED call edges.

**Cache integration**: Check `extraction_cache` table by `(file_path, content_hash)` before parsing. Skip files with cached results.

**.graphifyignore support**: Honors `.graphifyignore` file (gitignore syntax) in project root. Same behavior as Python version.

---

### graphify-build

Merge extractions into SQLite graph.

```rust
pub fn build(extractions: &[Extraction], db: &Connection) -> BuildResult

pub struct BuildResult {
    pub nodes_added: usize,
    pub edges_added: usize,
    pub duplicates_merged: usize,
}
```

- Inserts nodes/edges into SQLite
- Resolves cross-file references (imports, calls)
- Deduplicates nodes by id
- For incremental updates: deletes old nodes/edges for changed files before inserting new ones

---

### graphify-cluster

Community detection via label propagation.

```rust
pub fn cluster(db: &Connection) -> ClusterResult

pub struct ClusterResult {
    pub communities: HashMap<u32, usize>,
    pub iterations: u32,
}
```

- Loads node IDs + adjacency from SQLite into petgraph
- Runs label propagation algorithm
- Writes community assignments back to `nodes.community` in SQLite

---

### graphify-analyze

Graph analysis.

```rust
pub fn analyze(db: &Connection) -> AnalysisResult

pub struct AnalysisResult {
    pub god_nodes: Vec<NodeAnalysis>,          // top N by degree centrality
    pub surprising_connections: Vec<SurprisingEdge>,  // edges between communities
    pub suggested_questions: Vec<String>,       // based on structure
}
```

- Loads from SQLite into petgraph
- Computes degree centrality, writes to `nodes.degree_centrality`
- Identifies cross-community edges (surprising connections)
- Generates suggested questions from hub nodes + community structure

---

### graphify-report

Markdown report generation.

```rust
pub fn generate_report(db: &Connection, analysis: &AnalysisResult) -> String
```

- Reads from SQLite
- Formats analysis results into markdown
- Written to `.graphify/graph_report.md`

---

### graphify-napi

napi-rs bindings. Pure marshalling, no logic.

```rust
#[napi] pub fn run_pipeline(root: String) -> napi::Result<PipelineResultJs>
#[napi] pub fn query_graph(db_path: String, question: String, opts: QueryOpts) -> napi::Result<QueryResultJs>
#[napi] pub fn get_node(db_path: String, node_id: String) -> napi::Result<NodeJs>
#[napi] pub fn get_neighbors(db_path: String, node_id: String) -> napi::Result<Vec<NodeJs>>
#[napi] pub fn shortest_path(db_path: String, from: String, to: String) -> napi::Result<Vec<String>>
#[napi] pub fn graph_stats(db_path: String) -> napi::Result<GraphStatsJs>
#[napi] pub fn export_json(db_path: String, out_path: String) -> napi::Result<()>
```

---

### graphify-cli

Node.js CLI package.

```json
{
  "name": "@nodesify/graphify",
  "version": "0.1.0",
  "bin": { "graphify": "./dist/index.js" },
  "napi": {
    "name": "graphify",
    "triples": ["darwin-arm64", "darwin-x64", "linux-x64-gnu", "win32-x64-msvc"]
  }
}
```

**Commands v1:**

| Command | Purpose |
|---------|---------|
| `graphify run <path>` | Full pipeline |
| `graphify query "<question>"` | BFS/DFS traversal |
| `graphify path "A" "B"` | Shortest path |
| `graphify explain "X"` | Node explanation |
| `graphify stats` | Graph statistics |
| `graphify update <path>` | Incremental update |
| `graphify export` | Export to JSON |

---

## Error Handling

All crate functions return `Result<T, GraphifyError>` using `thiserror` for error types:

```rust
#[derive(Debug, thiserror::Error)]
pub enum GraphifyError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Parse error in {file}: {message}")]
    Parse { file: String, message: String },
    #[error("Graph error: {0}")]
    Graph(String),
}
```

No panics in library code. All errors propagate as `Result`.

---

## Dependencies

### Rust crates

| Crate | Purpose | Used by |
|-------|---------|---------|
| `tree-sitter` | AST parsing | graphify-extract |
| `tree-sitter-python`, `tree-sitter-javascript`, etc. | Language grammars | graphify-extract |
| `petgraph` | In-memory graph algorithms | graphify-cluster, graphify-analyze |
| `rusqlite` | SQLite bindings | graphify-core |
| `napi` + `napi-derive` | Node.js native addon | graphify-napi |
| `sha2` | File hashing | graphify-detect, graphify-extract |
| `serde` + `serde_json` | Serialization | graphify-core, graphify-report |
| `walkdir` | Filesystem walking | graphify-detect |
| `ignore` | .graphifyignore support | graphify-detect |
| `thiserror` | Error types | graphify-core |

### Node.js packages

| Package | Purpose |
|---------|---------|
| `@napi-rs/cli` | Build + publish native binaries |
| `commander` or `yargs` | CLI argument parsing |

---

## Testing Strategy

- **Rust unit tests**: One test module per crate, `#[cfg(test)]` inline
- **Integration tests**: `tests/` at workspace root with fixture files in 8 languages
- **Node.js tests**: Test each CLI command against fixture project
- **CI**: cargo test + npm test + napi build check on push

---

## Future Expansion (post-v1)

These are explicitly out of scope for v1 but the architecture supports them:

| Feature | How it fits |
|---------|-------------|
| HTML export | New crate `graphify-export-html`, add to pipeline |
| GraphML export | New crate `graphify-export-graphml` |
| Obsidian export | New crate `graphify-export-obsidian` |
| MCP server | New crate `graphify-serve`, reads from SQLite |
| Watch mode | New crate `graphify-watch`, calls `run_pipeline` on change |
| More languages | Add files to `graphify-extract/src/langs/` |
| LLM semantic extraction | New crate `graphify-semantic`, enriches graph |
| Video transcription | New crate `graphify-transcribe` |
| Skill system | Platform installers in `graphify-cli` |
| Neo4j export | New crate `graphify-export-neo4j` |
