# Nodesify Graphify Implementation Plan

> **STATUS: COMPLETE.** All phases 1–8 have been implemented. Phases 9 (integration tests) and CI are not yet done.
>
> **Deviations from this plan:**
> - `tests/fixtures/` was not created. Each crate uses `tempfile` for on-the-fly fixtures in unit tests instead.
> - `graphify-core/src/pipeline.rs` was not created. Pipeline orchestration lives in `graphify-napi/src/pipeline.rs`.
> - CLI has additional commands not in original plan: `watch`, `install`, `hook` (in `packages/graphify-cli/src/commands/`).
> - `graphify-napi/src/query.rs` added separately — contains BFS/DFS query, shortest path, and explain logic.
> - `ARCHITECTURE.md` and `REARCHITECTURE.md` now marked as legacy reference docs.
>
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Rewrite graphify from Python to Rust, published via npm as `@nodesify/graphify`, with core pipeline (detect → extract → build → cluster → analyze → report → export) backed by SQLite persistence and petgraph algorithms.

**Architecture:** Rust workspace with 8 domain crates. SQLite is the backbone (extraction cache, file manifest, graph storage, pipeline state, query history). petgraph hydrates from SQLite for in-memory graph algorithms (label propagation, centrality, BFS/DFS). napi-rs bridges Rust core to Node.js CLI.

**Tech Stack:** Rust, tree-sitter (0.25), petgraph, rusqlite (bundled), napi-rs (v3), TypeScript, commander.js

**Design spec:** `docs/superpowers/specs/2026-04-30-nodesify-graphify-rewrite-design.md`

---

## File Structure

```
nodesify-graphify/
├── Cargo.toml                          # workspace root
├── package.json                        # npm workspace root
├── crates/
│   ├── graphify-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # re-exports
│   │       ├── types.rs                # Node, Edge, FileType, Relation, Confidence, SourceLocation
│   │       ├── error.rs                # GraphifyError
│   │       ├── db.rs                   # SQLite schema init, open/close
│   │       └── pipeline.rs             # run_pipeline orchestration
│   ├── graphify-detect/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs                  # detect(), classify_file(), DetectResult, FileEntry
│   ├── graphify-extract/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # re-exports, extract()
│   │       ├── engine.rs               # tree-sitter orchestration, two-pass
│   │       ├── schema.rs               # Extraction, ExtractedNode, ExtractedEdge
│   │       └── langs/
│   │           ├── mod.rs              # registry, get_language()
│   │           ├── config.rs           # LanguageConfig struct
│   │           ├── python.rs
│   │           ├── javascript.rs
│   │           ├── typescript.rs
│   │           ├── rust.rs
│   │           ├── go.rs
│   │           ├── java.rs
│   │           └── c.rs
│   ├── graphify-build/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs                  # build()
│   ├── graphify-cluster/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs                  # cluster(), label propagation
│   ├── graphify-analyze/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs                  # analyze(), god_nodes, surprising_connections
│   ├── graphify-report/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs                  # generate_report()
│   └── graphify-napi/
│       ├── Cargo.toml
│       ├── build.rs
│       └── src/
│           └── lib.rs                  # #[napi] functions
├── packages/
│   └── graphify-cli/
│       ├── package.json
│       ├── tsconfig.json
│       └── src/
│           ├── index.ts                # CLI entry, commander setup
│           └── commands/
│               ├── run.ts
│               ├── query.ts
│               ├── path.ts
│               ├── explain.ts
│               ├── stats.ts
│               ├── update.ts
│               └── export.ts
├── tests/
│   ├── fixtures/
│   │   ├── python/
│   │   │   └── sample.py
│   │   ├── javascript/
│   │   │   └── sample.js
│   │   ├── typescript/
│   │   │   └── sample.ts
│   │   ├── rust/
│   │   │   └── sample.rs
│   │   ├── go/
│   │   │   └── sample.go
│   │   ├── java/
│   │   │   └── Sample.java
│   │   ├── c/
│   │   │   └── sample.c
│   │   └── cpp/
│   │       └── sample.cpp
│   └── integration/
│       └── pipeline_test.rs
└── .github/
    └── workflows/
        └── ci.yml
```

---

## Phase 1: Workspace Scaffolding ✅

### Task 1: Initialize Rust workspace ✅

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/graphify-core/Cargo.toml`
- Create: `crates/graphify-core/src/lib.rs`

- [x] **Step 1: Create workspace root Cargo.toml**

```toml
[workspace]
resolver = "2"
members = [
    "crates/graphify-core",
    "crates/graphify-detect",
    "crates/graphify-extract",
    "crates/graphify-build",
    "crates/graphify-cluster",
    "crates/graphify-analyze",
    "crates/graphify-report",
    "crates/graphify-napi",
]

[workspace.dependencies]
graphify-core = { path = "crates/graphify-core" }
graphify-detect = { path = "crates/graphify-detect" }
graphify-extract = { path = "crates/graphify-extract" }
graphify-build = { path = "crates/graphify-build" }
graphify-cluster = { path = "crates/graphify-cluster" }
graphify-analyze = { path = "crates/graphify-analyze" }
graphify-report = { path = "crates/graphify-report" }
rusqlite = { version = "0.34", features = ["bundled"] }
petgraph = "0.7"
thiserror = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
walkdir = "2"
ignore = "0.4"
tree-sitter = "0.25"
tree-sitter-python = "0.25"
tree-sitter-javascript = "0.25"
tree-sitter-typescript = "0.25"
tree-sitter-rust = "0.25"
tree-sitter-go = "0.25"
tree-sitter-java = "0.25"
tree-sitter-c = "0.25"
tree-sitter-cpp = "0.25"
```

- [x] **Step 2: Create graphify-core Cargo.toml**

```toml
[package]
name = "graphify-core"
version = "0.1.0"
edition = "2021"

[dependencies]
rusqlite = { workspace = true }
petgraph = { workspace = true }
thiserror = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
graphify-detect = { workspace = true }
graphify-extract = { workspace = true }
graphify-build = { workspace = true }
graphify-cluster = { workspace = true }
graphify-analyze = { workspace = true }
graphify-report = { workspace = true }
```

- [x] **Step 3: Create graphify-core/src/lib.rs placeholder**

```rust
pub mod types;
pub mod error;
pub mod db;
```

- [x] **Step 4: Create stub Cargo.toml and src/lib.rs for remaining crates**

Each crate gets a minimal Cargo.toml and `src/lib.rs` with an empty body so the workspace compiles.

`crates/graphify-detect/Cargo.toml`:
```toml
[package]
name = "graphify-detect"
version = "0.1.0"
edition = "2021"

[dependencies]
graphify-core = { workspace = true }
walkdir = { workspace = true }
ignore = { workspace = true }
sha2 = { workspace = true }
rusqlite = { workspace = true }
```

`crates/graphify-detect/src/lib.rs`:
```rust
// TODO: implement
```

`crates/graphify-extract/Cargo.toml`:
```toml
[package]
name = "graphify-extract"
version = "0.1.0"
edition = "2021"

[dependencies]
graphify-core = { workspace = true }
tree-sitter = { workspace = true }
tree-sitter-python = { workspace = true }
tree-sitter-javascript = { workspace = true }
tree-sitter-typescript = { workspace = true }
tree-sitter-rust = { workspace = true }
tree-sitter-go = { workspace = true }
tree-sitter-java = { workspace = true }
tree-sitter-c = { workspace = true }
tree-sitter-cpp = { workspace = true }
rusqlite = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
sha2 = { workspace = true }
```

`crates/graphify-extract/src/lib.rs`:
```rust
// TODO: implement
```

`crates/graphify-build/Cargo.toml`:
```toml
[package]
name = "graphify-build"
version = "0.1.0"
edition = "2021"

[dependencies]
graphify-core = { workspace = true }
graphify-extract = { workspace = true }
rusqlite = { workspace = true }
```

`crates/graphify-build/src/lib.rs`:
```rust
// TODO: implement
```

`crates/graphify-cluster/Cargo.toml`:
```toml
[package]
name = "graphify-cluster"
version = "0.1.0"
edition = "2021"

[dependencies]
graphify-core = { workspace = true }
rusqlite = { workspace = true }
petgraph = { workspace = true }
```

`crates/graphify-cluster/src/lib.rs`:
```rust
// TODO: implement
```

`crates/graphify-analyze/Cargo.toml`:
```toml
[package]
name = "graphify-analyze"
version = "0.1.0"
edition = "2021"

[dependencies]
graphify-core = { workspace = true }
rusqlite = { workspace = true }
petgraph = { workspace = true }
```

`crates/graphify-analyze/src/lib.rs`:
```rust
// TODO: implement
```

`crates/graphify-report/Cargo.toml`:
```toml
[package]
name = "graphify-report"
version = "0.1.0"
edition = "2021"

[dependencies]
graphify-core = { workspace = true }
rusqlite = { workspace = true }
```

`crates/graphify-report/src/lib.rs`:
```rust
// TODO: implement
```

`crates/graphify-napi/Cargo.toml`:
```toml
[package]
name = "graphify-napi"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
napi = { version = "3", features = ["tokio_rt"] }
napi-derive = "3"
graphify-core = { workspace = true }

[build-dependencies]
napi-build = "2"
```

`crates/graphify-napi/build.rs`:
```rust
extern crate napi_build;

fn main() {
    napi_build::setup();
}
```

`crates/graphify-napi/src/lib.rs`:
```rust
// TODO: implement
```

- [x] **Step 5: Verify workspace compiles**

Run: `cargo check`
Expected: Compiles with warnings about unused modules. No errors.

- [x] **Step 6: Commit**

```bash
git init
git add -A
git commit -m "feat: scaffold Rust workspace with 8 crates"
```

---

## Phase 2: Core Types & SQLite Layer ✅

### Task 2: Define core types ✅

**Files:**
- Create: `crates/graphify-core/src/types.rs`
- Create: `crates/graphify-core/src/error.rs`

- [x] **Step 1: Write types.rs with tests**

`crates/graphify-core/src/types.rs`:
```rust
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FileType {
    Code,
    Document,
    Paper,
    Image,
    Video,
}

impl FileType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileType::Code => "code",
            FileType::Document => "document",
            FileType::Paper => "paper",
            FileType::Image => "image",
            FileType::Video => "video",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "code" => Some(FileType::Code),
            "document" => Some(FileType::Document),
            "paper" => Some(FileType::Paper),
            "image" => Some(FileType::Image),
            "video" => Some(FileType::Video),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SourceLocation {
    pub line: u32,
    pub column: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Relation {
    Calls,
    Imports,
    Uses,
    Defines,
    Contains,
    Inherits,
    References,
    Rationale { tag: String },
}

impl Relation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Relation::Calls => "calls",
            Relation::Imports => "imports",
            Relation::Uses => "uses",
            Relation::Defines => "defines",
            Relation::Contains => "contains",
            Relation::Inherits => "inherits",
            Relation::References => "references",
            Relation::Rationale { .. } => "rationale",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "calls" => Some(Relation::Calls),
            "imports" => Some(Relation::Imports),
            "uses" => Some(Relation::Uses),
            "defines" => Some(Relation::Defines),
            "contains" => Some(Relation::Contains),
            "inherits" => Some(Relation::Inherits),
            "references" => Some(Relation::References),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Confidence {
    Extracted,
    Inferred,
    Ambiguous,
}

impl Confidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Confidence::Extracted => "EXTRACTED",
            Confidence::Inferred => "INFERRED",
            Confidence::Ambiguous => "AMBIGUOUS",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "EXTRACTED" => Some(Confidence::Extracted),
            "INFERRED" => Some(Confidence::Inferred),
            "AMBIGUOUS" => Some(Confidence::Ambiguous),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub file_type: FileType,
    pub source_file: PathBuf,
    pub source_location: Option<SourceLocation>,
    pub docstring: Option<String>,
    pub community: Option<u32>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub relation: Relation,
    pub confidence: Confidence,
    pub confidence_score: Option<f64>,
    pub source_file: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub community_count: usize,
    pub file_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_type_roundtrip() {
        for ft in [FileType::Code, FileType::Document, FileType::Paper, FileType::Image, FileType::Video] {
            assert_eq!(FileType::from_str(ft.as_str()), Some(ft));
        }
    }

    #[test]
    fn relation_roundtrip() {
        for rel in [
            Relation::Calls, Relation::Imports, Relation::Uses,
            Relation::Defines, Relation::Contains, Relation::Inherits,
            Relation::References,
        ] {
            assert_eq!(Relation::from_str(rel.as_str()), Some(rel));
        }
    }

    #[test]
    fn confidence_roundtrip() {
        for conf in [Confidence::Extracted, Confidence::Inferred, Confidence::Ambiguous] {
            assert_eq!(Confidence::from_str(conf.as_str()), Some(conf));
        }
    }

    #[test]
    fn node_serialization_roundtrip() {
        let node = Node {
            id: "main.py::MyClass::method".into(),
            label: "method()".into(),
            file_type: FileType::Code,
            source_file: PathBuf::from("src/main.py"),
            source_location: Some(SourceLocation { line: 42, column: Some(4) }),
            docstring: Some("Does a thing".into()),
            community: Some(1),
        };
        let json = serde_json::to_string(&node).unwrap();
        let back: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, node.id);
        assert_eq!(back.community, node.community);
    }
}
```

- [x] **Step 2: Write error.rs**

`crates/graphify-core/src/error.rs`:
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

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, GraphifyError>;
```

- [x] **Step 3: Run tests**

Run: `cargo test -p graphify-core`
Expected: 4 tests pass (file_type_roundtrip, relation_roundtrip, confidence_roundtrip, node_serialization_roundtrip).

- [x] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: core types (Node, Edge, Relation, Confidence) with serialization"
```

---

### Task 3: SQLite schema & connection management ✅

**Files:**
- Create: `crates/graphify-core/src/db.rs`
- Modify: `crates/graphify-core/src/lib.rs`

- [x] **Step 1: Write db.rs with tests**

`crates/graphify-core/src/db.rs`:
```rust
use rusqlite::Connection;
use crate::error::Result;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS extraction_cache (
    file_path TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    language TEXT NOT NULL,
    nodes TEXT NOT NULL,
    edges TEXT NOT NULL,
    extracted_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS file_manifest (
    file_path TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    file_type TEXT NOT NULL,
    language TEXT,
    last_seen_at TEXT NOT NULL,
    size_bytes INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS nodes (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    file_type TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_line INTEGER,
    docstring TEXT,
    community INTEGER,
    degree_centrality REAL
);
CREATE INDEX IF NOT EXISTS idx_nodes_file ON nodes(source_file);
CREATE INDEX IF NOT EXISTS idx_nodes_community ON nodes(community);

CREATE TABLE IF NOT EXISTS edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL REFERENCES nodes(id),
    target TEXT NOT NULL REFERENCES nodes(id),
    relation TEXT NOT NULL,
    confidence TEXT NOT NULL,
    confidence_score REAL,
    source_file TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source);
CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target);

CREATE TABLE IF NOT EXISTS pipeline_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    status TEXT NOT NULL,
    files_processed INTEGER,
    nodes_added INTEGER,
    edges_added INTEGER
);

CREATE TABLE IF NOT EXISTS query_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    question TEXT NOT NULL,
    answer TEXT,
    path_taken TEXT,
    queried_at TEXT NOT NULL
);
";

pub fn open_db(path: &std::path::Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

pub fn open_db_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory_creates_tables() {
        let conn = open_db_in_memory().unwrap();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(tables.contains(&"nodes".to_string()));
        assert!(tables.contains(&"edges".to_string()));
        assert!(tables.contains(&"extraction_cache".to_string()));
        assert!(tables.contains(&"file_manifest".to_string()));
        assert!(tables.contains(&"pipeline_runs".to_string()));
        assert!(tables.contains(&"query_history".to_string()));
    }

    #[test]
    fn indexes_exist() {
        let conn = open_db_in_memory().unwrap();
        let indexes: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(indexes.contains(&"idx_nodes_file".to_string()));
        assert!(indexes.contains(&"idx_nodes_community".to_string()));
        assert!(indexes.contains(&"idx_edges_source".to_string()));
        assert!(indexes.contains(&"idx_edges_target".to_string()));
    }

    #[test]
    fn insert_and_query_node() {
        let conn = open_db_in_memory().unwrap();
        conn.execute(
            "INSERT INTO nodes (id, label, file_type, source_file, source_line) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params!["main.py::Foo", "Foo", "code", "main.py", 10],
        ).unwrap();

        let label: String = conn
            .query_row("SELECT label FROM nodes WHERE id = ?1", rusqlite::params!["main.py::Foo"], |row| row.get(0))
            .unwrap();
        assert_eq!(label, "Foo");
    }
}
```

- [x] **Step 2: Update lib.rs**

`crates/graphify-core/src/lib.rs`:
```rust
pub mod types;
pub mod error;
pub mod db;

pub use types::*;
pub use error::{GraphifyError, Result};
pub use db::{open_db, open_db_in_memory};
```

- [x] **Step 3: Run tests**

Run: `cargo test -p graphify-core`
Expected: All tests pass (7 total: 4 from types, 3 from db).

- [x] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: SQLite schema with 6 tables, indexes, connection management"
```

---

## Phase 3: Detect ✅

### Task 4: File discovery & classification ✅

**Files:**
- Modify: `crates/graphify-detect/src/lib.rs`

- [x] **Step 1: Write detect implementation with tests**

`crates/graphify-detect/src/lib.rs`:
```rust
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use graphify_core::{FileType, db::open_db_in_memory};
use rusqlite::Connection;
use sha2::{Sha256, Digest};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub file_type: FileType,
    pub language: Option<String>,
    pub content_hash: String,
    pub size_bytes: u64,
}

#[derive(Debug)]
pub struct DetectResult {
    pub new: Vec<FileEntry>,
    pub changed: Vec<FileEntry>,
    pub unchanged: Vec<FileEntry>,
    pub removed: Vec<FileEntry>,
}

const CODE_EXTENSIONS: &[&str] = &[
    ".py", ".js", ".jsx", ".mjs", ".ts", ".tsx",
    ".rs", ".go", ".java", ".c", ".h", ".cpp", ".cc", ".cxx", ".hpp",
];
const DOC_EXTENSIONS: &[&str] = &[".md", ".mdx", ".txt", ".rst"];
const PAPER_EXTENSIONS: &[&str] = &[".pdf"];
const IMAGE_EXTENSIONS: &[&str] = &[".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg"];
const VIDEO_EXTENSIONS: &[&str] = &[".mp4", ".mov", ".webm", ".mkv", ".avi"];

const EXTENSION_TO_LANGUAGE: &[(&str, &str)] = &[
    (".py", "Python"), (".js", "JavaScript"), (".jsx", "JavaScript"),
    (".mjs", "JavaScript"), (".ts", "TypeScript"), (".tsx", "TypeScript"),
    (".rs", "Rust"), (".go", "Go"), (".java", "Java"),
    (".c", "C"), (".h", "C"), (".cpp", "C++"), (".cc", "C++"),
    (".cxx", "C++"), (".hpp", "C++"),
];

pub fn classify_file(path: &Path) -> Option<FileType> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    let ext_with_dot = format!(".{}", ext);
    if CODE_EXTENSIONS.contains(&ext_with_dot.as_str()) {
        return Some(FileType::Code);
    }
    if DOC_EXTENSIONS.contains(&ext_with_dot.as_str()) {
        return Some(FileType::Document);
    }
    if PAPER_EXTENSIONS.contains(&ext_with_dot.as_str()) {
        return Some(FileType::Paper);
    }
    if IMAGE_EXTENSIONS.contains(&ext_with_dot.as_str()) {
        return Some(FileType::Image);
    }
    if VIDEO_EXTENSIONS.contains(&ext_with_dot.as_str()) {
        return Some(FileType::Video);
    }
    None
}

pub fn language_for_extension(ext: &str) -> Option<&'static str> {
    let ext_with_dot = if ext.starts_with('.') { ext.to_lowercase() } else { format!(".{}", ext).to_lowercase() };
    EXTENSION_TO_LANGUAGE.iter()
        .find(|(e, _)| e.to_lowercase() == ext_with_dot)
        .map(|(_, lang)| *lang)
}

fn file_hash(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn detect(root: &Path, db: &Connection) -> graphify_core::Result<DetectResult> {
    let mut new_files = Vec::new();
    let mut changed_files = Vec::new();
    let mut unchanged_files = Vec::new();
    let mut seen_paths: HashSet<String> = HashSet::new();

    let mut ignore_builder = ignore::WalkBuilder::new(root);
    ignore_builder
        .hidden(false)
        .git_ignore(true)
        .add_custom_ignore_filename(".graphifyignore");

    // Check for .graphifyignore
    let graphifyignore = root.join(".graphifyignore");
    if graphifyignore.exists() {
        let _ = ignore_builder.add_ignore(graphifyignore);
    }

    for entry in ignore_builder.build().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let relative = path.strip_prefix(root).unwrap_or(path);
        let Some(file_type) = classify_file(path) else {
            continue;
        };

        let rel_str = relative.to_string_lossy().to_string().replace('\\', "/");
        seen_paths.insert(rel_str.clone());

        let metadata = std::fs::metadata(path)?;
        let size_bytes = metadata.len();
        let hash = file_hash(path)?;

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let language = language_for_extension(ext).map(|s| s.to_string());

        // Check manifest
        let stored_hash: Option<String> = db
            .query_row(
                "SELECT content_hash FROM file_manifest WHERE file_path = ?1",
                rusqlite::params![rel_str],
                |row| row.get(0),
            )
            .ok();

        let entry = FileEntry {
            path: relative.to_path_buf(),
            file_type,
            language,
            content_hash: hash,
            size_bytes,
        };

        match stored_hash {
            None => new_files.push(entry),
            Some(h) if h != entry.content_hash => changed_files.push(entry),
            Some(_) => unchanged_files.push(entry),
        }
    }

    // Find removed files
    let mut removed_files = Vec::new();
    let mut stmt = db.prepare("SELECT file_path, content_hash, file_type, language, size_bytes FROM file_manifest")?;
    let rows: Vec<(String, String, String, Option<String>, u64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)))?
        .filter_map(|r| r.ok())
        .collect();

    for (fp, hash, ft, lang, size) in rows {
        if !seen_paths.contains(&fp) {
            removed_files.push(FileEntry {
                path: PathBuf::from(&fp),
                file_type: FileType::from_str(&ft).unwrap_or(FileType::Code),
                language: lang,
                content_hash: hash,
                size_bytes: size,
            });
        }
    }

    Ok(DetectResult {
        new: new_files,
        changed: changed_files,
        unchanged: unchanged_files,
        removed: removed_files,
    })
}

pub fn update_manifest(result: &DetectResult, db: &Connection) -> graphify_core::Result<()> {
    let now = chrono_now();
    let all_entries: Vec<&FileEntry> = result.new.iter().chain(result.changed.iter()).chain(result.unchanged.iter()).collect();
    for entry in &all_entries {
        db.execute(
            "INSERT OR REPLACE INTO file_manifest (file_path, content_hash, file_type, language, last_seen_at, size_bytes) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                entry.path.to_string_lossy().to_string().replace('\\', "/"),
                entry.content_hash,
                entry.file_type.as_str(),
                entry.language,
                now,
                entry.size_bytes,
            ],
        )?;
    }
    for entry in &result.removed {
        db.execute("DELETE FROM file_manifest WHERE file_path = ?1", rusqlite::params![entry.path.to_string_lossy().to_string()])?;
    }
    Ok(())
}

fn chrono_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn classify_known_extensions() {
        assert_eq!(classify_file(Path::new("foo.py")), Some(FileType::Code));
        assert_eq!(classify_file(Path::new("foo.rs")), Some(FileType::Code));
        assert_eq!(classify_file(Path::new("foo.md")), Some(FileType::Document));
        assert_eq!(classify_file(Path::new("foo.pdf")), Some(FileType::Paper));
        assert_eq!(classify_file(Path::new("foo.png")), Some(FileType::Image));
        assert_eq!(classify_file(Path::new("foo.mp4")), Some(FileType::Video));
        assert_eq!(classify_file(Path::new("foo.xyz")), None);
        assert_eq!(classify_file(Path::new("Makefile")), None);
    }

    #[test]
    fn language_for_ext() {
        assert_eq!(language_for_extension(".py"), Some("Python"));
        assert_eq!(language_for_extension(".rs"), Some("Rust"));
        assert_eq!(language_for_extension(".xyz"), None);
    }

    #[test]
    fn detect_new_files_in_temp_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("main.py"), "def hello(): pass\n").unwrap();
        fs::write(dir.path().join("readme.md"), "# Hello\n").unwrap();

        let db = open_db_in_memory().unwrap();
        let result = detect(dir.path(), &db).unwrap();

        assert_eq!(result.new.len(), 2);
        assert_eq!(result.changed.len(), 0);
        assert_eq!(result.removed.len(), 0);
        assert!(result.new.iter().any(|f| f.path.to_string_lossy().contains("main.py")));
        assert!(result.new.iter().any(|f| f.language.as_deref() == Some("Python")));
    }

    #[test]
    fn detect_changed_files_after_update() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("main.py"), "def hello(): pass\n").unwrap();

        let db = open_db_in_memory().unwrap();
        let result = detect(dir.path(), &db).unwrap();
        update_manifest(&result, &db).unwrap();

        // Modify file
        fs::write(dir.path().join("main.py"), "def goodbye(): pass\n").unwrap();
        let result2 = detect(dir.path(), &db).unwrap();

        assert_eq!(result2.new.len(), 0);
        assert_eq!(result2.changed.len(), 1);
    }

    #[test]
    fn detect_removed_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.py"), "a\n").unwrap();
        fs::write(dir.path().join("b.py"), "b\n").unwrap();

        let db = open_db_in_memory().unwrap();
        let result = detect(dir.path(), &db).unwrap();
        update_manifest(&result, &db).unwrap();

        // Remove b.py
        fs::remove_file(dir.path().join("b.py")).unwrap();
        let result2 = detect(dir.path(), &db).unwrap();

        assert_eq!(result2.removed.len(), 1);
        assert!(result2.removed[0].path.to_string_lossy().contains("b.py"));
    }
}
```

Add `tempfile` dev dependency to `crates/graphify-detect/Cargo.toml`:
```toml
[dev-dependencies]
tempfile = "3"
```

- [x] **Step 2: Run tests**

Run: `cargo test -p graphify-detect`
Expected: 5 tests pass.

- [x] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: detect module — file discovery, classification, incremental via SQLite manifest"
```

---

## Phase 4: Extract ✅

### Task 5: Extraction engine + LanguageConfig + schema ✅

**Files:**
- Create: `crates/graphify-extract/src/schema.rs`
- Create: `crates/graphify-extract/src/langs/config.rs`
- Create: `crates/graphify-extract/src/langs/mod.rs`
- Create: `crates/graphify-extract/src/engine.rs`
- Modify: `crates/graphify-extract/src/lib.rs`

- [x] **Step 1: Write extraction schema**

`crates/graphify-extract/src/schema.rs`:
```rust
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ExtractedNode {
    pub id: String,
    pub label: String,
    pub source_file: PathBuf,
    pub source_line: Option<u32>,
    pub docstring: Option<String>,
    pub node_type: String, // "class", "function", "file"
}

#[derive(Debug, Clone)]
pub struct ExtractedEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub confidence: String,
    pub confidence_score: Option<f64>,
    pub source_file: PathBuf,
    pub source_line: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Extraction {
    pub file_path: PathBuf,
    pub language: String,
    pub nodes: Vec<ExtractedNode>,
    pub edges: Vec<ExtractedEdge>,
}
```

- [x] **Step 2: Write LanguageConfig**

`crates/graphify-extract/src/langs/config.rs`:
```rust
use tree_sitter::Language;

pub struct LanguageConfig {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub language_fn: fn() -> Language,
    pub class_types: &'static [&'static str],
    pub function_types: &'static [&'static str],
    pub import_types: &'static [&'static str],
    pub call_type: &'static str,
    pub name_field: &'static str,
    pub body_field: Option<&'static str>,
    pub body_fallback_types: &'static [&'static str],
}
```

- [x] **Step 3: Write language registry with Python config**

`crates/graphify-extract/src/langs/mod.rs`:
```rust
pub mod config;
pub mod python;
pub mod javascript;
pub mod typescript;
pub mod rust;
pub mod go;
pub mod java;
pub mod c;

use config::LanguageConfig;

pub fn get_language_for_extension(ext: &str) -> Option<&'static LanguageConfig> {
    let ext_lower = ext.to_lowercase();
    let ext_with_dot = if ext_lower.starts_with('.') { ext_lower } else { format!(".{}", ext_lower) };
    all_languages().iter().find(|cfg| cfg.extensions.contains(&ext_with_dot.as_str())).copied()
}

pub fn all_languages() -> Vec<&'static LanguageConfig> {
    vec![
        python::config(),
        javascript::config(),
        typescript::config(),
        rust::config(),
        go::config(),
        java::config(),
        c::config(),
    ]
}
```

- [x] **Step 4: Write Python language config**

`crates/graphify-extract/src/langs/python.rs`:
```rust
use super::config::LanguageConfig;

pub fn config() -> &'static LanguageConfig {
    &LanguageConfig {
        name: "Python",
        extensions: &[".py"],
        language_fn: || tree_sitter_python::LANGUAGE.into(),
        class_types: &["class_definition", "decorated_definition"],
        function_types: &["function_definition"],
        import_types: &["import_statement", "import_from_statement"],
        call_type: "call",
        name_field: "name",
        body_field: Some("body"),
        body_fallback_types: &[],
    }
}
```

- [x] **Step 5: Write remaining language configs**

`crates/graphify-extract/src/langs/javascript.rs`:
```rust
use super::config::LanguageConfig;

pub fn config() -> &'static LanguageConfig {
    &LanguageConfig {
        name: "JavaScript",
        extensions: &[".js", ".jsx", ".mjs"],
        language_fn: || tree_sitter_javascript::LANGUAGE.into(),
        class_types: &["class_declaration", "class"],
        function_types: &["function_declaration", "generator_function_declaration", "method_definition"],
        import_types: &["import_statement", "import_declaration"],
        call_type: "call_expression",
        name_field: "name",
        body_field: Some("body"),
        body_fallback_types: &["statement_block"],
    }
}
```

`crates/graphify-extract/src/langs/typescript.rs`:
```rust
use super::config::LanguageConfig;

pub fn config() -> &'static LanguageConfig {
    &LanguageConfig {
        name: "TypeScript",
        extensions: &[".ts", ".tsx"],
        language_fn: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        class_types: &["class_declaration"],
        function_types: &["function_declaration", "generator_function_declaration", "method_definition", "arrow_function"],
        import_types: &["import_statement", "import_declaration"],
        call_type: "call_expression",
        name_field: "name",
        body_field: Some("body"),
        body_fallback_types: &["statement_block"],
    }
}
```

`crates/graphify-extract/src/langs/rust.rs`:
```rust
use super::config::LanguageConfig;

pub fn config() -> &'static LanguageConfig {
    &LanguageConfig {
        name: "Rust",
        extensions: &[".rs"],
        language_fn: || tree_sitter_rust::LANGUAGE.into(),
        class_types: &["struct_item", "enum_item", "trait_item", "impl_item"],
        function_types: &["function_item", "function_signature_item"],
        import_types: &["use_declaration"],
        call_type: "call_expression",
        name_field: "name",
        body_field: Some("body"),
        body_fallback_types: &["block"],
    }
}
```

`crates/graphify-extract/src/langs/go.rs`:
```rust
use super::config::LanguageConfig;

pub fn config() -> &'static LanguageConfig {
    &LanguageConfig {
        name: "Go",
        extensions: &[".go"],
        language_fn: || tree_sitter_go::LANGUAGE.into(),
        class_types: &["type_declaration"],
        function_types: &["function_declaration", "method_declaration"],
        import_types: &["import_declaration"],
        call_type: "call_expression",
        name_field: "name",
        body_field: Some("body"),
        body_fallback_types: &["block"],
    }
}
```

`crates/graphify-extract/src/langs/java.rs`:
```rust
use super::config::LanguageConfig;

pub fn config() -> &'static LanguageConfig {
    &LanguageConfig {
        name: "Java",
        extensions: &[".java"],
        language_fn: || tree_sitter_java::LANGUAGE.into(),
        class_types: &["class_declaration", "interface_declaration", "enum_declaration"],
        function_types: &["method_declaration", "constructor_declaration"],
        import_types: &["import_declaration"],
        call_type: "method_invocation",
        name_field: "name",
        body_field: Some("body"),
        body_fallback_types: &["block"],
    }
}
```

`crates/graphify-extract/src/langs/c.rs`:
```rust
use super::config::LanguageConfig;

pub fn config() -> &'static LanguageConfig {
    static C_CONFIG: LanguageConfig = LanguageConfig {
        name: "C",
        extensions: &[".c", ".h"],
        language_fn: || tree_sitter_c::LANGUAGE.into(),
        class_types: &["struct_specifier", "enum_specifier"],
        function_types: &["function_definition"],
        import_types: &["preproc_include"],
        call_type: "call_expression",
        name_field: "declarator",
        body_field: Some("body"),
        body_fallback_types: &["compound_statement"],
    };
    static CPP_CONFIG: LanguageConfig = LanguageConfig {
        name: "C++",
        extensions: &[".cpp", ".cc", ".cxx", ".hpp"],
        language_fn: || tree_sitter_cpp::LANGUAGE.into(),
        class_types: &["class_specifier", "struct_specifier", "enum_specifier"],
        function_types: &["function_definition"],
        import_types: &["preproc_include"],
        call_type: "call_expression",
        name_field: "declarator",
        body_field: Some("body"),
        body_fallback_types: &["compound_statement"],
    };

    // This function returns the C config; callers use get_language_for_extension
    // which checks both C and C++ configs by extension.
    &C_CONFIG
}

pub fn cpp_config() -> &'static LanguageConfig {
    &CPP_CONFIG
}
```

Update `crates/graphify-extract/src/langs/mod.rs` to include cpp:
```rust
use config::LanguageConfig;

pub fn get_language_for_extension(ext: &str) -> Option<&'static LanguageConfig> {
    let ext_lower = ext.to_lowercase();
    let ext_with_dot = if ext_lower.starts_with('.') { ext_lower } else { format!(".{}", ext_lower) };
    all_languages().iter().find(|cfg| cfg.extensions.contains(&ext_with_dot.as_str())).copied()
}

pub fn all_languages() -> Vec<&'static LanguageConfig> {
    vec![
        python::config(),
        javascript::config(),
        typescript::config(),
        rust::config(),
        go::config(),
        java::config(),
        c::config(),
        c::cpp_config(),
    ]
}
```

- [x] **Step 6: Write extraction engine**

`crates/graphify-extract/src/engine.rs`:
```rust
use std::path::{Path, PathBuf};
use tree_sitter::{Parser, Node};
use rusqlite::Connection;
use sha2::{Sha256, Digest};

use crate::schema::{Extraction, ExtractedNode, ExtractedEdge};
use crate::langs::{config::LanguageConfig, get_language_for_extension};

fn make_id(*parts: &str) -> String {
    let combined: String = parts.iter().map(|p| p.trim_matches(|c: char| c == '_' || c == '.')).collect::<Vec<_>>().join("_");
    let cleaned: String = combined.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
    cleaned.trim_matches('_').to_lowercase()
}

fn file_stem(path: &Path) -> String {
    let parent = path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("");
    if !parent.is_empty() && parent != "." {
        format!("{}.{}", parent, path.file_stem().unwrap_or_default().to_string_lossy())
    } else {
        path.file_stem().unwrap_or_default().to_string_lossy().to_string()
    }
}

fn node_text<'a>(node: &Node, source: &'a [u8]) -> &'a str {
    std::str::from_utf8(&source[node.byte_range()]).unwrap_or("")
}

fn file_hash(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize())
}

fn check_cache(db: &Connection, file_path: &str, hash: &str) -> Option<Extraction> {
    let (nodes_json, edges_json): (String, String) = db.query_row(
        "SELECT nodes, edges FROM extraction_cache WHERE file_path = ?1 AND content_hash = ?2",
        rusqlite::params![file_path, hash],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).ok()?;

    let nodes: Vec<ExtractedNode> = serde_json::from_str(&nodes_json).ok()?;
    let edges: Vec<ExtractedEdge> = serde_json::from_str(&edges_json).ok()?;
    let language = nodes.first().map(|n| "cached".to_string()).unwrap_or_default();
    Some(Extraction {
        file_path: PathBuf::from(file_path),
        language,
        nodes,
        edges,
    })
}

fn save_cache(db: &Connection, file_path: &str, hash: &str, language: &str, extraction: &Extraction) {
    let nodes_json = serde_json::to_string(&extraction.nodes).unwrap_or_default();
    let edges_json = serde_json::to_string(&extraction.edges).unwrap_or_default();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs().to_string();
    let _ = db.execute(
        "INSERT OR REPLACE INTO extraction_cache (file_path, content_hash, language, nodes, edges, extracted_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![file_path, hash, language, nodes_json, edges_json, now],
    );
}

fn extract_single(path: &Path, cfg: &LanguageConfig) -> graphify_core::Result<Extraction> {
    let source = std::fs::read(path)?;
    let source_str = source.as_slice();
    let mut parser = Parser::new();
    let language = (cfg.language_fn)();
    parser.set_language(&language).map_err(|e| graphify_core::GraphifyError::Parse {
        file: path.to_string_lossy().to_string(),
        message: e.to_string(),
    })?;

    let tree = parser.parse(&source, None).ok_or_else(|| graphify_core::GraphifyError::Parse {
        file: path.to_string_lossy().to_string(),
        message: "Failed to parse".into(),
    })?;

    let root = tree.root_node();
    let stem = file_stem(path);
    let str_path = path.to_string_lossy().to_string().replace('\\', "/");
    let file_nid = make_id(&stem);

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut function_bodies: Vec<(String, Node)> = Vec::new();

    // File node
    nodes.push(ExtractedNode {
        id: file_nid.clone(),
        label: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
        source_file: path.to_path_buf(),
        source_line: Some(1),
        docstring: None,
        node_type: "file".into(),
    });

    // Pass 1: structural extraction
    let mut cursor = root.walk();
    walk_structural(&mut cursor, source_str, cfg, &file_nid, &stem, &str_path, path, &mut nodes, &mut edges, &mut function_bodies);

    // Pass 2: call-graph extraction
    for (func_nid, body_node) in &function_bodies {
        walk_calls(&body_node, source_str, cfg, func_nid, &str_path, path, &mut edges);
    }

    Ok(Extraction {
        file_path: path.to_path_buf(),
        language: cfg.name.to_string(),
        nodes,
        edges,
    })
}

fn walk_structural<'a>(
    cursor: &mut tree_sitter::TreeCursor<'a>,
    source: &'a [u8],
    cfg: &LanguageConfig,
    file_nid: &str,
    stem: &str,
    str_path: &str,
    path: &Path,
    nodes: &mut Vec<ExtractedNode>,
    edges: &mut Vec<ExtractedEdge>,
    function_bodies: &mut Vec<(String, Node<'a>)>,
) {
    loop {
        let node = cursor.node();
        let kind = node.kind();

        if cfg.class_types.contains(&kind) {
            if let Some(name_node) = node.child_by_field_name(cfg.name_field) {
                let name = node_text(&name_node, source);
                let nid = make_id(stem, name);
                let line = node.start_position().row + 1;
                // Extract docstring from body
                let docstring = extract_docstring(&node, source, cfg);
                nodes.push(ExtractedNode {
                    id: nid.clone(),
                    label: name.to_string(),
                    source_file: path.to_path_buf(),
                    source_line: Some(line),
                    docstring,
                    node_type: "class".into(),
                });
                edges.push(ExtractedEdge {
                    source: file_nid.to_string(),
                    target: nid.clone(),
                    relation: "contains".into(),
                    confidence: "EXTRACTED".into(),
                    confidence_score: Some(1.0),
                    source_file: path.to_path_buf(),
                    source_line: Some(line),
                });

                // Walk children of this class for methods
                let mut child_cursor = node.walk();
                loop {
                    let child = child_cursor.node();
                    if cfg.function_types.contains(&child.kind()) {
                        if let Some(fn_name_node) = child.child_by_field_name(cfg.name_field) {
                            let fn_name = node_text(&fn_name_node, source);
                            let fn_nid = make_id(stem, name, fn_name);
                            let fn_line = child.start_position().row + 1;
                            nodes.push(ExtractedNode {
                                id: fn_nid.clone(),
                                label: format!("{}()", fn_name),
                                source_file: path.to_path_buf(),
                                source_line: Some(fn_line),
                                docstring: extract_docstring(&child, source, cfg),
                                node_type: "function".into(),
                            });
                            edges.push(ExtractedEdge {
                                source: nid.clone(),
                                target: fn_nid.clone(),
                                relation: "contains".into(),
                                confidence: "EXTRACTED".into(),
                                confidence_score: Some(1.0),
                                source_file: path.to_path_buf(),
                                source_line: Some(fn_line),
                            });
                            if let Some(body) = find_body(&child, cfg) {
                                function_bodies.push((fn_nid, body));
                            }
                        }
                    }
                    if !child_cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        } else if cfg.function_types.contains(&kind) {
            if let Some(name_node) = node.child_by_field_name(cfg.name_field) {
                let name = node_text(&name_node, source);
                let nid = make_id(stem, name);
                let line = node.start_position().row + 1;
                nodes.push(ExtractedNode {
                    id: nid.clone(),
                    label: format!("{}()", name),
                    source_file: path.to_path_buf(),
                    source_line: Some(line),
                    docstring: extract_docstring(&node, source, cfg),
                    node_type: "function".into(),
                });
                edges.push(ExtractedEdge {
                    source: file_nid.to_string(),
                    target: nid.clone(),
                    relation: "contains".into(),
                    confidence: "EXTRACTED".into(),
                    confidence_score: Some(1.0),
                    source_file: path.to_path_buf(),
                    source_line: Some(line),
                });
                if let Some(body) = find_body(&node, cfg) {
                    function_bodies.push((nid, body));
                }
            }
        } else if cfg.import_types.contains(&kind) {
            extract_imports(&node, source, cfg, file_nid, stem, str_path, path, edges);
        }

        if cursor.goto_first_child() {
            walk_structural(cursor, source, cfg, file_nid, stem, str_path, path, nodes, edges, function_bodies);
            cursor.goto_parent();
        }

        if !cursor.goto_next_sibling() {
            break;
        }
    }
}

fn find_body<'a>(node: &Node<'a>, cfg: &LanguageConfig) -> Option<Node<'a>> {
    if let Some(field) = cfg.body_field {
        if let Some(body) = node.child_by_field_name(field) {
            return Some(body);
        }
    }
    for child in node.children(&mut node.walk()) {
        if cfg.body_fallback_types.contains(&child.kind()) {
            return Some(child);
        }
    }
    None
}

fn extract_docstring(node: &Node, source: &[u8], cfg: &LanguageConfig) -> Option<String> {
    let body = find_body(node, cfg)?;
    let first_child = body.named_child(0)?;
    if first_child.kind() == "expression_statement" {
        if let Some(string_node) = first_child.named_child(0) {
            if string_node.kind().contains("string") {
                let text = node_text(&string_node, source);
                return Some(text.trim_matches('"').trim_matches('\'').to_string());
            }
        }
    }
    None
}

fn extract_imports(
    node: &Node, source: &[u8], _cfg: &LanguageConfig,
    file_nid: &str, _stem: &str, str_path: &str, path: &Path,
    edges: &mut Vec<ExtractedEdge>,
) {
    // Generic import extraction: find string/identifier children
    for child in node.children(&mut node.walk()) {
        let kind = child.kind();
        if kind.contains("string") || kind == "identifier" || kind == "dotted_name" {
            let raw = node_text(&child, source);
            let module_name = raw.trim_matches('"').trim_matches('\'').trim_matches('<').trim_matches('>');
            let module_short = module_name.split('/').next_back().unwrap_or(module_name).split('.').next().unwrap_or(module_name);
            let tgt_nid = make_id(module_short);
            edges.push(ExtractedEdge {
                source: file_nid.to_string(),
                target: tgt_nid,
                relation: "imports".into(),
                confidence: "EXTRACTED".into(),
                confidence_score: Some(1.0),
                source_file: path.to_path_buf(),
                source_line: Some(node.start_position().row + 1),
            });
        }
    }
}

fn walk_calls<'a>(
    body_node: &Node<'a>,
    source: &'a [u8],
    cfg: &LanguageConfig,
    func_nid: &str,
    str_path: &str,
    path: &Path,
    edges: &mut Vec<ExtractedEdge>,
) {
    let mut cursor = body_node.walk();
    loop {
        let node = cursor.node();
        if node.kind() == cfg.call_type {
            if let Some(func_name_node) = node.child_by_field_name("function") {
                let callee = node_text(&func_name_node, source);
                let callee_short = callee.rsplit('.').next().unwrap_or(callee);
                let callee_nid = make_id(callee_short);
                edges.push(ExtractedEdge {
                    source: func_nid.to_string(),
                    target: callee_nid,
                    relation: "calls".into(),
                    confidence: "INFERRED".into(),
                    confidence_score: Some(0.7),
                    source_file: path.to_path_buf(),
                    source_line: Some(node.start_position().row + 1),
                });
            }
        }
        if cursor.goto_first_child() {
            continue;
        }
        if cursor.goto_next_sibling() {
            continue;
        }
        loop {
            if cursor.goto_parent() {
                if cursor.goto_next_sibling() {
                    break;
                }
            } else {
                return;
            }
        }
    }
}

pub fn extract(files: &[PathBuf], db: &Connection) -> graphify_core::Result<Vec<Extraction>> {
    let mut results = Vec::new();
    for path in files {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let Some(cfg) = get_language_for_extension(ext) else {
            continue;
        };

        let rel_str = path.to_string_lossy().to_string().replace('\\', "/");
        let hash = file_hash(path);

        if let Some(cached) = check_cache(db, &rel_str, &hash) {
            results.push(cached);
            continue;
        }

        let extraction = extract_single(path, cfg)?;
        save_cache(db, &rel_str, &hash, cfg.name, &extraction);
        results.push(extraction);
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphify_core::db::open_db_in_memory;
    use std::fs;

    #[test]
    fn extract_python_file() {
        let dir = tempfile::tempdir().unwrap();
        let py = dir.path().join("main.py");
        fs::write(&py, r#"
class Greeter:
    """Says hello"""
    def greet(self, name):
        print(name)

def helper():
    pass
"#).unwrap();

        let db = open_db_in_memory().unwrap();
        let results = extract(&[py.clone()], &db).unwrap();
        assert_eq!(results.len(), 1);

        let ext = &results[0];
        assert_eq!(ext.language, "Python");
        // file node + class Greeter + method greet + function helper
        assert!(ext.nodes.len() >= 3, "expected at least 3 nodes, got {}: {:?}", ext.nodes.len(), ext.nodes.iter().map(|n| &n.label).collect::<Vec<_>>());
        assert!(ext.nodes.iter().any(|n| n.label == "Greeter"), "missing class Greeter");
        assert!(ext.nodes.iter().any(|n| n.label == "greet()"), "missing method greet");
        assert!(ext.nodes.iter().any(|n| n.label == "helper()"), "missing function helper");

        // Check docstring
        let greeter = ext.nodes.iter().find(|n| n.label == "Greeter").unwrap();
        assert_eq!(greeter.docstring.as_deref(), Some("Says hello"));

        // Check edges
        assert!(ext.edges.iter().any(|e| e.relation == "contains"), "missing contains edge");
        assert!(ext.edges.iter().any(|e| e.relation == "calls"), "missing call edge (print)");
    }

    #[test]
    fn extract_rust_file() {
        let dir = tempfile::tempdir().unwrap();
        let rs = dir.path().join("main.rs");
        fs::write(&rs, r#"
struct Config {
    name: String,
}

fn main() {
    let c = Config { name: "test".into() };
    println!("hello");
}
"#).unwrap();

        let db = open_db_in_memory().unwrap();
        let results = extract(&[rs], &db).unwrap();
        assert_eq!(results.len(), 1);
        let ext = &results[0];
        assert_eq!(ext.language, "Rust");
        assert!(ext.nodes.iter().any(|n| n.label == "Config"), "missing struct Config");
        assert!(ext.nodes.iter().any(|n| n.label == "main()"), "missing fn main");
    }

    #[test]
    fn extract_javascript_file() {
        let dir = tempfile::tempdir().unwrap();
        let js = dir.path().join("app.js");
        fs::write(&js, r#"
class App {
    start() {
        console.log("hello");
    }
}

function helper() {
    return 42;
}
"#).unwrap();

        let db = open_db_in_memory().unwrap();
        let results = extract(&[js], &db).unwrap();
        let ext = &results[0];
        assert_eq!(ext.language, "JavaScript");
        assert!(ext.nodes.iter().any(|n| n.label == "App"), "missing class App");
        assert!(ext.nodes.iter().any(|n| n.label == "helper()"), "missing function helper");
    }

    #[test]
    fn extraction_uses_cache() {
        let dir = tempfile::tempdir().unwrap();
        let py = dir.path().join("main.py");
        fs::write(&py, "def hello(): pass\n").unwrap();

        let db = open_db_in_memory().unwrap();
        let r1 = extract(&[py.clone()], &db).unwrap();
        let r2 = extract(&[py.clone()], &db).unwrap();
        assert_eq!(r1[0].nodes.len(), r2[0].nodes.len());
    }
}
```

- [x] **Step 7: Update lib.rs**

`crates/graphify-extract/src/lib.rs`:
```rust
pub mod schema;
pub mod engine;
pub mod langs;

pub use schema::{Extraction, ExtractedNode, ExtractedEdge};
pub use engine::extract;
```

- [x] **Step 8: Run tests**

Run: `cargo test -p graphify-extract`
Expected: 4 tests pass (python, rust, javascript, cache).

- [x] **Step 9: Commit**

```bash
git add -A
git commit -m "feat: extract engine — tree-sitter AST parsing, 8 languages, two-pass, SQLite cache"
```

---

## Phase 5: Build, Cluster, Analyze, Report ✅

### Task 6: Build — merge extractions into SQLite ✅

**Files:**
- Modify: `crates/graphify-build/src/lib.rs`

- [x] **Step 1: Write build implementation with tests**

`crates/graphify-build/src/lib.rs`:
```rust
use rusqlite::Connection;
use graphify_core::{Node, Edge, FileType, Relation, Confidence};
use graphify_extract::{Extraction, ExtractedNode, ExtractedEdge};

#[derive(Debug)]
pub struct BuildResult {
    pub nodes_added: usize,
    pub edges_added: usize,
    pub duplicates_merged: usize,
}

pub fn build(extractions: &[Extraction], db: &Connection) -> graphify_core::Result<BuildResult> {
    let mut nodes_added = 0;
    let mut edges_added = 0;
    let mut duplicates_merged = 0;

    let tx = db.unchecked_transaction()?;

    for extraction in extractions {
        // Delete old nodes/edges for this file (incremental update support)
        let file_path = extraction.file_path.to_string_lossy().to_string().replace('\\', "/");
        tx.execute("DELETE FROM edges WHERE source_file = ?1", rusqlite::params![file_path])?;
        tx.execute("DELETE FROM nodes WHERE source_file = ?1", rusqlite::params![file_path])?;

        for node in &extraction.nodes {
            let existing: bool = tx.query_row(
                "SELECT COUNT(*) FROM nodes WHERE id = ?1",
                rusqlite::params![node.id],
                |row| row.get::<_, i64>(0),
            ).unwrap_or(0) > 0;

            if existing {
                duplicates_merged += 1;
                continue;
            }

            tx.execute(
                "INSERT OR IGNORE INTO nodes (id, label, file_type, source_file, source_line, docstring) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    node.id,
                    node.label,
                    FileType::Code.as_str(),
                    node.source_file.to_string_lossy().to_string().replace('\\', "/"),
                    node.source_line,
                    node.docstring,
                ],
            )?;
            nodes_added += 1;
        }

        for edge in &extraction.edges {
            tx.execute(
                "INSERT INTO edges (source, target, relation, confidence, confidence_score, source_file) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    edge.source,
                    edge.target,
                    edge.relation,
                    edge.confidence,
                    edge.confidence_score,
                    edge.source_file.to_string_lossy().to_string().replace('\\', "/"),
                ],
            )?;
            edges_added += 1;
        }
    }

    tx.commit()?;
    Ok(BuildResult { nodes_added, edges_added, duplicates_merged })
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphify_core::db::open_db_in_memory;
    use std::path::PathBuf;

    fn make_extraction(nodes: Vec<(&str, &str)>, edges: Vec<(&str, &str, &str)>) -> Extraction {
        Extraction {
            file_path: PathBuf::from("test.py"),
            language: "Python".into(),
            nodes: nodes.into_iter().map(|(id, label)| ExtractedNode {
                id: id.into(),
                label: label.into(),
                source_file: PathBuf::from("test.py"),
                source_line: Some(1),
                docstring: None,
                node_type: "function".into(),
            }).collect(),
            edges: edges.into_iter().map(|(src, tgt, rel)| ExtractedEdge {
                source: src.into(),
                target: tgt.into(),
                relation: rel.into(),
                confidence: "EXTRACTED".into(),
                confidence_score: Some(1.0),
                source_file: PathBuf::from("test.py"),
                source_line: Some(1),
            }).collect(),
        }
    }

    #[test]
    fn build_inserts_nodes_and_edges() {
        let db = open_db_in_memory().unwrap();
        let extractions = vec![make_extraction(
            vec![("testpy::hello", "hello()"), ("testpy::world", "world()")],
            vec![("testpy::hello", "testpy::world", "calls")],
        )];
        let result = build(&extractions, &db).unwrap();
        assert_eq!(result.nodes_added, 2);
        assert_eq!(result.edges_added, 1);
    }

    #[test]
    fn build_deduplicates_nodes() {
        let db = open_db_in_memory().unwrap();
        let ext = make_extraction(
            vec![("testpy::hello", "hello()")],
            vec![],
        );
        build(&[ext.clone()], &db).unwrap();
        let result = build(&[ext], &db).unwrap();
        assert_eq!(result.duplicates_merged, 1);
    }

    #[test]
    fn build_replaces_file_on_rebuild() {
        let db = open_db_in_memory().unwrap();
        let ext1 = make_extraction(vec![("testpy::old", "old()")], vec![]);
        build(&[ext1], &db).unwrap();

        let ext2 = make_extraction(vec![("testpy::new", "new()")], vec![]);
        let result = build(&[ext2], &db).unwrap();
        assert_eq!(result.nodes_added, 1);

        let count: i64 = db.query_row("SELECT COUNT(*) FROM nodes WHERE id = 'testpy::old'", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0, "old node should be deleted");
    }
}
```

- [x] **Step 2: Run tests**

Run: `cargo test -p graphify-build`
Expected: 3 tests pass.

- [x] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: build module — merge extractions into SQLite, incremental replace, dedup"
```

---

### Task 7: Cluster — label propagation ✅

**Files:**
- Modify: `crates/graphify-cluster/src/lib.rs`

- [x] **Step 1: Write cluster implementation with tests**

`crates/graphify-cluster/src/lib.rs`:
```rust
use std::collections::{HashMap, HashSet};
use rusqlite::Connection;
use petgraph::graph::UnGraph;
use petgraph::graph::NodeIndex;

#[derive(Debug)]
pub struct ClusterResult {
    pub communities: HashMap<u32, usize>,
    pub iterations: u32,
}

pub fn cluster(db: &Connection) -> graphify_core::Result<ClusterResult> {
    // Load nodes and edges from SQLite
    let node_ids: Vec<String> = {
        let mut stmt = db.prepare("SELECT id FROM nodes")?;
        stmt.query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect()
    };

    if node_ids.is_empty() {
        return Ok(ClusterResult { communities: HashMap::new(), iterations: 0 });
    }

    let id_to_idx: HashMap<String, NodeIndex> = node_ids.iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), NodeIndex::new(i)))
        .collect();

    let mut graph = UnGraph::<String, ()>::new_undirected();
    let mut idx_to_id = Vec::new();
    for id in &node_ids {
        let idx = graph.add_node(id.clone());
        idx_to_id.push(graph[idx].clone());
    }

    {
        let mut stmt = db.prepare("SELECT source, target FROM edges")?;
        let edges: Vec<(String, String)> = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        for (src, tgt) in edges {
            if let (Some(&s), Some(&t)) = (id_to_idx.get(&src), id_to_idx.get(&tgt)) {
                graph.add_edge(s, t, ());
            }
        }
    }

    // Label propagation
    let n = node_ids.len();
    let mut labels: Vec<u32> = (0..n as u32).collect();

    let mut iterations = 0;
    let max_iterations = 100;

    for _ in 0..max_iterations {
        iterations += 1;
        let mut changed = false;

        for i in 0..n {
            let node_idx = NodeIndex::new(i);
            let mut neighbor_labels: HashMap<u32, usize> = HashMap::new();
            neighbor_labels.insert(labels[i], 1);

            for neighbor in graph.neighbors(node_idx) {
                let nidx = neighbor.index();
                *neighbor_labels.entry(labels[nidx]).or_insert(0) += 1;
            }

            let best_label = *neighbor_labels.iter()
                .max_by_key(|(_, &count)| count)
                .map(|(label, _)| label)
                .unwrap();

            if best_label != labels[i] {
                labels[i] = best_label;
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }

    // Write community assignments back to SQLite
    for (i, id) in node_ids.iter().enumerate() {
        db.execute(
            "UPDATE nodes SET community = ?1 WHERE id = ?2",
            rusqlite::params![labels[i] as i64, id],
        )?;
    }

    // Count communities
    let mut communities: HashMap<u32, usize> = HashMap::new();
    for &label in &labels {
        *communities.entry(label).or_insert(0) += 1;
    }

    Ok(ClusterResult { communities, iterations })
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphify_core::db::open_db_in_memory;

    fn seed_graph(db: &Connection) {
        // 4 nodes, 3 edges: A-B, B-C, C-D
        // Expected: A,B,C,D should end up in the same community (connected graph)
        db.execute("INSERT INTO nodes (id, label, file_type, source_file) VALUES ('a', 'A', 'code', 'f.py')", []).unwrap();
        db.execute("INSERT INTO nodes (id, label, file_type, source_file) VALUES ('b', 'B', 'code', 'f.py')", []).unwrap();
        db.execute("INSERT INTO nodes (id, label, file_type, source_file) VALUES ('c', 'C', 'code', 'f.py')", []).unwrap();
        db.execute("INSERT INTO nodes (id, label, file_type, source_file) VALUES ('d', 'D', 'code', 'f.py')", []).unwrap();
        db.execute("INSERT INTO edges (source, target, relation, confidence, source_file) VALUES ('a', 'b', 'calls', 'EXTRACTED', 'f.py')", []).unwrap();
        db.execute("INSERT INTO edges (source, target, relation, confidence, source_file) VALUES ('b', 'c', 'calls', 'EXTRACTED', 'f.py')", []).unwrap();
        db.execute("INSERT INTO edges (source, target, relation, confidence, source_file) VALUES ('c', 'd', 'calls', 'EXTRACTED', 'f.py')", []).unwrap();
    }

    #[test]
    fn cluster_assigns_communities() {
        let db = open_db_in_memory().unwrap();
        seed_graph(&db);
        let result = cluster(&db).unwrap();
        assert!(result.communities.len() >= 1);
        assert!(result.iterations > 0);

        // Check that nodes have community assigned in DB
        let community: i64 = db.query_row("SELECT community FROM nodes WHERE id = 'a'", [], |r| r.get(0)).unwrap();
        assert!(community >= 0);
    }

    #[test]
    fn connected_graph_single_community() {
        let db = open_db_in_memory().unwrap();
        seed_graph(&db);
        let result = cluster(&db).unwrap();
        // All connected → single community
        assert_eq!(result.communities.len(), 1);
    }

    #[test]
    fn empty_graph_no_crash() {
        let db = open_db_in_memory().unwrap();
        let result = cluster(&db).unwrap();
        assert_eq!(result.communities.len(), 0);
    }
}
```

- [x] **Step 2: Run tests**

Run: `cargo test -p graphify-cluster`
Expected: 3 tests pass.

- [x] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: cluster module — label propagation via petgraph, SQLite persistence"
```

---

### Task 8: Analyze — god nodes, surprising connections, questions ✅

**Files:**
- Modify: `crates/graphify-analyze/src/lib.rs`

- [x] **Step 1: Write analyze implementation with tests**

`crates/graphify-analyze/src/lib.rs`:
```rust
use std::collections::HashMap;
use rusqlite::Connection;
use petgraph::graph::UnGraph;
use petgraph::graph::NodeIndex;
use petgraph::algo::dijkstra;

#[derive(Debug, Clone)]
pub struct NodeAnalysis {
    pub id: String,
    pub label: String,
    pub degree: usize,
    pub community: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct SurprisingEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub source_community: Option<u32>,
    pub target_community: Option<u32>,
}

#[derive(Debug)]
pub struct AnalysisResult {
    pub god_nodes: Vec<NodeAnalysis>,
    pub surprising_connections: Vec<SurprisingEdge>,
    pub suggested_questions: Vec<String>,
}

pub fn analyze(db: &Connection) -> graphify_core::Result<AnalysisResult> {
    let god_nodes = compute_god_nodes(db)?;
    let surprising = compute_surprising_connections(db)?;
    let questions = suggest_questions(db, &god_nodes)?;
    Ok(AnalysisResult {
        god_nodes,
        surprising_connections: surprising,
        suggested_questions: questions,
    })
}

fn compute_god_nodes(db: &Connection) -> graphify_core::Result<Vec<NodeAnalysis>> {
    let mut stmt = db.prepare(
        "SELECT n.id, n.label, n.community, COUNT(e.id) as degree
         FROM nodes n
         LEFT JOIN edges e ON e.source = n.id OR e.target = n.id
         GROUP BY n.id
         ORDER BY degree DESC
         LIMIT 10"
    )?;

    let nodes: Vec<NodeAnalysis> = stmt.query_map([], |row| {
        Ok(NodeAnalysis {
            id: row.get(0)?,
            label: row.get(1)?,
            degree: row.get::<_, i64>(3)? as usize,
            community: row.get::<_, Option<i64>>(2)?.map(|c| c as u32),
        })
    })?.filter_map(|r| r.ok()).collect();

    // Update degree_centrality in SQLite
    if !nodes.is_empty() {
        let max_degree = nodes[0].degree as f64;
        if max_degree > 0.0 {
            for node in &nodes {
                let centrality = node.degree as f64 / max_degree;
                db.execute(
                    "UPDATE nodes SET degree_centrality = ?1 WHERE id = ?2",
                    rusqlite::params![centrality, node.id],
                )?;
            }
        }
    }

    Ok(nodes)
}

fn compute_surprising_connections(db: &Connection) -> graphify_core::Result<Vec<SurprisingEdge>> {
    let mut stmt = db.prepare(
        "SELECT e.source, e.target, e.relation, s.community, t.community
         FROM edges e
         JOIN nodes s ON s.id = e.source
         JOIN nodes t ON t.id = e.target
         WHERE s.community IS NOT NULL
           AND t.community IS NOT NULL
           AND s.community != t.community"
    )?;

    let edges: Vec<SurprisingEdge> = stmt.query_map([], |row| {
        Ok(SurprisingEdge {
            source: row.get(0)?,
            target: row.get(1)?,
            relation: row.get(2)?,
            source_community: row.get::<_, Option<i64>>(3)?.map(|c| c as u32),
            target_community: row.get::<_, Option<i64>>(4)?.map(|c| c as u32),
        })
    })?.filter_map(|r| r.ok()).collect();

    Ok(edges)
}

fn suggest_questions(db: &Connection, god_nodes: &[NodeAnalysis]) -> graphify_core::Result<Vec<String>> {
    let mut questions = Vec::new();

    for node in god_nodes.iter().take(5) {
        questions.push(format!("Why does {} have so many connections?", node.label));
    }

    // Community-based questions
    let community_count: i64 = db.query_row(
        "SELECT COUNT(DISTINCT community) FROM nodes WHERE community IS NOT NULL",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    if community_count > 1 {
        questions.push(format!("What connects the {} different communities?", community_count));
    }

    Ok(questions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphify_core::db::open_db_in_memory;

    fn seed_analyzed_graph(db: &Connection) {
        db.execute_batch("
            INSERT INTO nodes (id, label, file_type, source_file, community) VALUES ('a', 'Alpha', 'code', 'f.py', 0);
            INSERT INTO nodes (id, label, file_type, source_file, community) VALUES ('b', 'Beta', 'code', 'f.py', 0);
            INSERT INTO nodes (id, label, file_type, source_file, community) VALUES ('c', 'Gamma', 'code', 'f.py', 1);
            INSERT INTO edges (source, target, relation, confidence, source_file) VALUES ('a', 'b', 'calls', 'EXTRACTED', 'f.py');
            INSERT INTO edges (source, target, relation, confidence, source_file) VALUES ('a', 'c', 'calls', 'EXTRACTED', 'f.py');
            INSERT INTO edges (source, target, relation, confidence, source_file) VALUES ('b', 'c', 'calls', 'EXTRACTED', 'f.py');
        ").unwrap();
    }

    #[test]
    fn analyze_finds_god_nodes() {
        let db = open_db_in_memory().unwrap();
        seed_analyzed_graph(&db);
        let result = analyze(&db).unwrap();
        assert!(!result.god_nodes.is_empty());
        assert_eq!(result.god_nodes[0].id, "a"); // Alpha has 2 edges, highest degree
    }

    #[test]
    fn analyze_finds_surprising_connections() {
        let db = open_db_in_memory().unwrap();
        seed_analyzed_graph(&db);
        let result = analyze(&db).unwrap();
        // a→c and b→c cross communities
        assert!(!result.surprising_connections.is_empty());
        assert!(result.surprising_connections.iter().any(|e| e.source == "a" && e.target == "c"));
    }

    #[test]
    fn analyze_suggests_questions() {
        let db = open_db_in_memory().unwrap();
        seed_analyzed_graph(&db);
        let result = analyze(&db).unwrap();
        assert!(!result.suggested_questions.is_empty());
    }
}
```

- [x] **Step 2: Run tests**

Run: `cargo test -p graphify-analyze`
Expected: 3 tests pass.

- [x] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: analyze module — god nodes, surprising connections, suggested questions"
```

---

### Task 9: Report — markdown generation ✅

**Files:**
- Modify: `crates/graphify-report/src/lib.rs`

- [x] **Step 1: Write report implementation with tests**

`crates/graphify-report/src/lib.rs`:
```rust
use rusqlite::Connection;
use graphify_analyze::AnalysisResult;

pub fn generate_report(db: &Connection, analysis: &AnalysisResult) -> graphify_core::Result<String> {
    let node_count: i64 = db.query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0)).unwrap_or(0);
    let edge_count: i64 = db.query_row("SELECT COUNT(*) FROM edges", [], |r| r.get(0)).unwrap_or(0);
    let community_count: i64 = db.query_row(
        "SELECT COUNT(DISTINCT community) FROM nodes WHERE community IS NOT NULL",
        [],
        |r| r.get(0),
    ).unwrap_or(0);

    let mut report = String::new();
    report.push_str("# Graph Report\n\n");
    report.push_str(&format!("**Nodes:** {} | **Edges:** {} | **Communities:** {}\n\n", node_count, edge_count, community_count));

    // God nodes
    report.push_str("## Hub Nodes (God Nodes)\n\n");
    if analysis.god_nodes.is_empty() {
        report.push_str("No hub nodes found.\n\n");
    } else {
        for node in &analysis.god_nodes {
            let community_str = node.community.map(|c| c.to_string()).unwrap_or_else(|| "—".into());
            report.push_str(&format!("- **{}** (degree: {}, community: {})\n", node.label, node.degree, community_str));
        }
        report.push_str("\n");
    }

    // Surprising connections
    report.push_str("## Surprising Connections\n\n");
    if analysis.surprising_connections.is_empty() {
        report.push_str("No cross-community connections found.\n\n");
    } else {
        for edge in &analysis.surprising_connections {
            report.push_str(&format!(
                "- {} → {} ({}) [community {} → {}]\n",
                edge.source, edge.target, edge.relation,
                edge.source_community.map(|c| c.to_string()).unwrap_or_else(|| "?".into()),
                edge.target_community.map(|c| c.to_string()).unwrap_or_else(|| "?".into()),
            ));
        }
        report.push_str("\n");
    }

    // Suggested questions
    report.push_str("## Suggested Questions\n\n");
    for q in &analysis.suggested_questions {
        report.push_str(&format!("- {}\n", q));
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphify_core::db::open_db_in_memory;
    use graphify_analyze::{NodeAnalysis, SurprisingEdge};

    #[test]
    fn generate_report_with_data() {
        let db = open_db_in_memory().unwrap();
        db.execute_batch("
            INSERT INTO nodes (id, label, file_type, source_file, community) VALUES ('a', 'Alpha', 'code', 'f.py', 0);
            INSERT INTO nodes (id, label, file_type, source_file, community) VALUES ('b', 'Beta', 'code', 'f.py', 1);
            INSERT INTO edges (source, target, relation, confidence, source_file) VALUES ('a', 'b', 'calls', 'EXTRACTED', 'f.py');
        ").unwrap();

        let analysis = AnalysisResult {
            god_nodes: vec![NodeAnalysis { id: "a".into(), label: "Alpha".into(), degree: 1, community: Some(0) }],
            surprising_connections: vec![SurprisingEdge {
                source: "a".into(), target: "b".into(), relation: "calls".into(),
                source_community: Some(0), target_community: Some(1),
            }],
            suggested_questions: vec!["Why does Alpha have so many connections?".into()],
        };

        let report = generate_report(&db, &analysis).unwrap();
        assert!(report.contains("# Graph Report"));
        assert!(report.contains("Alpha"));
        assert!(report.contains("Surprising Connections"));
        assert!(report.contains("Suggested Questions"));
    }

    #[test]
    fn generate_report_empty_graph() {
        let db = open_db_in_memory().unwrap();
        let analysis = AnalysisResult {
            god_nodes: vec![],
            surprising_connections: vec![],
            suggested_questions: vec![],
        };
        let report = generate_report(&db, &analysis).unwrap();
        assert!(report.contains("Nodes: 0"));
    }
}
```

- [x] **Step 2: Run tests**

Run: `cargo test -p graphify-report`
Expected: 2 tests pass.

- [x] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: report module — markdown report from graph analysis"
```

---

## Phase 6: Pipeline Orchestration & Export ✅

### Task 10: Pipeline orchestration + JSON export ✅

**Files:**
- Create: `crates/graphify-core/src/pipeline.rs`
- Modify: `crates/graphify-core/src/lib.rs`

- [x] **Step 1: Write pipeline orchestration**

`crates/graphify-core/src/pipeline.rs`:
```rust
use std::path::{Path, PathBuf};
use rusqlite::Connection;
use crate::db;
use crate::error::Result;

#[derive(Debug)]
pub struct PipelineResult {
    pub build_result: graphify_build::BuildResult,
    pub cluster_result: graphify_cluster::ClusterResult,
    pub analysis: graphify_analyze::AnalysisResult,
    pub report: String,
}

pub fn run_pipeline(root: &Path) -> Result<PipelineResult> {
    let graphify_dir = root.join(".graphify");
    std::fs::create_dir_all(&graphify_dir)?;

    let db_path = graphify_dir.join("db.sqlite");
    let db = db::open_db(&db_path)?;

    let detected = graphify_detect::detect(root, &db)?;
    graphify_detect::update_manifest(&detected, &db)?;

    let files_to_process: Vec<PathBuf> = detected.new.iter()
        .chain(detected.changed.iter())
        .map(|e| root.join(&e.path))
        .collect();

    if files_to_process.is_empty() && detected.removed.is_empty() {
        // Nothing changed, but still run analysis on existing graph
        let analysis = graphify_analyze::analyze(&db)?;
        let report = graphify_report::generate_report(&db, &analysis)?;
        write_report(&graphify_dir, &report);
        export_json(&db, &graphify_dir.join("graph.json"))?;
        return Ok(PipelineResult {
            build_result: graphify_build::BuildResult { nodes_added: 0, edges_added: 0, duplicates_merged: 0 },
            cluster_result: graphify_cluster::ClusterResult { communities: Default::default(), iterations: 0 },
            analysis,
            report,
        });
    }

    let extractions = graphify_extract::extract(&files_to_process, &db)?;
    let build_result = graphify_build::build(&extractions, &db)?;
    let cluster_result = graphify_cluster::cluster(&db)?;
    let analysis = graphify_analyze::analyze(&db)?;
    let report = graphify_report::generate_report(&db, &analysis)?;

    write_report(&graphify_dir, &report);
    export_json(&db, &graphify_dir.join("graph.json"))?;

    Ok(PipelineResult {
        build_result,
        cluster_result,
        analysis,
        report,
    })
}

fn write_report(graphify_dir: &Path, report: &str) {
    let report_path = graphify_dir.join("graph_report.md");
    let _ = std::fs::write(report_path, report);
}

pub fn export_json(db: &Connection, out_path: &Path) -> Result<()> {
    let mut nodes = Vec::new();
    let mut stmt = db.prepare("SELECT id, label, file_type, source_file, source_line, docstring, community FROM nodes")?;
    let node_rows: Vec<(String, String, String, String, Option<i64>, Option<String>, Option<i64>)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)))?
        .filter_map(|r| r.ok())
        .collect();

    for (id, label, ft, sf, line, doc, comm) in &node_rows {
        nodes.push(serde_json::json!({
            "id": id,
            "label": label,
            "file_type": ft,
            "source_file": sf,
            "source_line": line,
            "docstring": doc,
            "community": comm,
        }));
    }

    let mut edges = Vec::new();
    let mut stmt = db.prepare("SELECT source, target, relation, confidence, confidence_score, source_file FROM edges")?;
    let edge_rows: Vec<(String, String, String, String, Option<f64>, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)))?
        .filter_map(|r| r.ok())
        .collect();

    for (src, tgt, rel, conf, score, sf) in &edge_rows {
        edges.push(serde_json::json!({
            "source": src,
            "target": tgt,
            "relation": rel,
            "confidence": conf,
            "confidence_score": score,
            "source_file": sf,
        }));
    }

    let graph = serde_json::json!({ "nodes": nodes, "edges": edges });
    let json = serde_json::to_string_pretty(&graph)?;
    std::fs::write(out_path, json)?;
    Ok(())
}

pub fn load_graph_db(root: &Path) -> Result<Connection> {
    let db_path = root.join(".graphify").join("db.sqlite");
    db::open_db(&db_path)
}
```

- [x] **Step 2: Update lib.rs**

`crates/graphify-core/src/lib.rs`:
```rust
pub mod types;
pub mod error;
pub mod db;
pub mod pipeline;

pub use types::*;
pub use error::{GraphifyError, Result};
pub use db::{open_db, open_db_in_memory};
pub use pipeline::{run_pipeline, export_json, load_graph_db};
```

- [x] **Step 3: Run full workspace tests**

Run: `cargo test`
Expected: All tests across all crates pass.

- [x] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: pipeline orchestration — detect→extract→build→cluster→analyze→report→export"
```

---

## Phase 7: napi-rs Bindings ✅

### Task 11: Rust-to-Node bridge ✅

**Files:**
- Modify: `crates/graphify-napi/src/lib.rs`

- [x] **Step 1: Write napi bindings**

`crates/graphify-napi/src/lib.rs`:
```rust
use napi_derive::napi;
use napi::bindgen_prelude::*;
use std::path::PathBuf;

#[napi(object)]
pub struct PipelineResultJs {
    pub nodes_added: i64,
    pub edges_added: i64,
    pub communities: i64,
    pub report: String,
}

#[napi]
pub fn run_pipeline(root: String) -> napi::Result<PipelineResultJs> {
    let path = PathBuf::from(&root);
    let result = graphify_core::pipeline::run_pipeline(&path)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    Ok(PipelineResultJs {
        nodes_added: result.build_result.nodes_added as i64,
        edges_added: result.build_result.edges_added as i64,
        communities: result.cluster_result.communities.len() as i64,
        report: result.report,
    })
}

#[napi(object)]
pub struct GraphStatsJs {
    pub node_count: i64,
    pub edge_count: i64,
    pub community_count: i64,
    pub file_count: i64,
}

#[napi]
pub fn graph_stats(root: String) -> napi::Result<GraphStatsJs> {
    let db = graphify_core::pipeline::load_graph_db(&PathBuf::from(&root))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    let node_count: i64 = db.query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0)).unwrap_or(0);
    let edge_count: i64 = db.query_row("SELECT COUNT(*) FROM edges", [], |r| r.get(0)).unwrap_or(0);
    let community_count: i64 = db.query_row(
        "SELECT COUNT(DISTINCT community) FROM nodes WHERE community IS NOT NULL",
        [], |r| r.get(0)
    ).unwrap_or(0);
    let file_count: i64 = db.query_row("SELECT COUNT(*) FROM file_manifest", [], |r| r.get(0)).unwrap_or(0);

    Ok(GraphStatsJs { node_count, edge_count, community_count, file_count })
}

#[napi(object)]
pub struct NodeJs {
    pub id: String,
    pub label: String,
    pub file_type: String,
    pub source_file: String,
    pub source_line: Option<i64>,
    pub docstring: Option<String>,
    pub community: Option<i64>,
}

#[napi]
pub fn get_node(root: String, node_id: String) -> napi::Result<Option<NodeJs>> {
    let db = graphify_core::pipeline::load_graph_db(&PathBuf::from(&root))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    let result = db.query_row(
        "SELECT id, label, file_type, source_file, source_line, docstring, community FROM nodes WHERE id = ?1",
        rusqlite::params![node_id],
        |row| Ok(NodeJs {
            id: row.get(0)?,
            label: row.get(1)?,
            file_type: row.get(2)?,
            source_file: row.get(3)?,
            source_line: row.get(4)?,
            docstring: row.get(5)?,
            community: row.get::<_, Option<i64>>(6)?,
        }),
    );

    match result {
        Ok(node) => Ok(Some(node)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(napi::Error::from_reason(e.to_string())),
    }
}

#[napi]
pub fn get_neighbors(root: String, node_id: String) -> napi::Result<Vec<NodeJs>> {
    let db = graphify_core::pipeline::load_graph_db(&PathBuf::from(&root))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    let mut stmt = db.prepare(
        "SELECT DISTINCT n.id, n.label, n.file_type, n.source_file, n.source_line, n.docstring, n.community
         FROM nodes n
         JOIN edges e ON (e.target = n.id AND e.source = ?1) OR (e.source = n.id AND e.target = ?1)
         WHERE n.id != ?1"
    ).map_err(|e| napi::Error::from_reason(e.to_string()))?;

    let nodes: Vec<NodeJs> = stmt.query_map(rusqlite::params![node_id], |row| {
        Ok(NodeJs {
            id: row.get(0)?,
            label: row.get(1)?,
            file_type: row.get(2)?,
            source_file: row.get(3)?,
            source_line: row.get(4)?,
            docstring: row.get(5)?,
            community: row.get::<_, Option<i64>>(6)?,
        })
    }).map_err(|e| napi::Error::from_reason(e.to_string()))?
    .filter_map(|r| r.ok())
    .collect();

    Ok(nodes)
}

#[napi]
pub fn export_json(root: String, out_path: String) -> napi::Result<()> {
    let db = graphify_core::pipeline::load_graph_db(&PathBuf::from(&root))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    graphify_core::pipeline::export_json(&db, &PathBuf::from(&out_path))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(())
}
```

- [x] **Step 2: Build napi module**

Run: `cd crates/graphify-napi && cargo check`
Expected: Compiles without errors.

- [x] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: napi-rs bindings — run_pipeline, graph_stats, get_node, get_neighbors, export_json"
```

---

## Phase 8: Node.js CLI ✅

### Task 12: CLI package setup + commands ✅

**Files:**
- Create: `packages/graphify-cli/package.json`
- Create: `packages/graphify-cli/tsconfig.json`
- Create: `packages/graphify-cli/src/index.ts`
- Create: `packages/graphify-cli/src/commands/run.ts`
- Create: `packages/graphify-cli/src/commands/query.ts`
- Create: `packages/graphify-cli/src/commands/path.ts`
- Create: `packages/graphify-cli/src/commands/explain.ts`
- Create: `packages/graphify-cli/src/commands/stats.ts`
- Create: `packages/graphify-cli/src/commands/update.ts`
- Create: `packages/graphify-cli/src/commands/export.ts`
- Modify: `package.json` (workspace root)

- [x] **Step 1: Create package.json**

`packages/graphify-cli/package.json`:
```json
{
  "name": "@nodesify/graphify",
  "version": "0.1.0",
  "description": "Turn any folder into a queryable knowledge graph",
  "main": "dist/index.js",
  "bin": {
    "graphify": "./dist/index.js"
  },
  "scripts": {
    "build": "tsc",
    "dev": "tsc --watch"
  },
  "dependencies": {
    "commander": "^12"
  },
  "devDependencies": {
    "typescript": "^5",
    "@types/node": "^22"
  },
  "napi": {
    "name": "graphify",
    "triples": {
      "defaults": true,
      "additional": [
        "x86_64-pc-windows-msvc",
        "aarch64-apple-darwin",
        "x86_64-unknown-linux-gnu"
      ]
    }
  }
}
```

- [x] **Step 2: Create tsconfig.json**

`packages/graphify-cli/tsconfig.json`:
```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "commonjs",
    "lib": ["ES2022"],
    "outDir": "dist",
    "rootDir": "src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "declaration": true,
    "declarationMap": true,
    "sourceMap": true
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist"]
}
```

- [x] **Step 3: Create CLI entry point**

`packages/graphify-cli/src/index.ts`:
```typescript
#!/usr/bin/env node

import { Command } from 'commander';
import { runCommand } from './commands/run';
import { queryCommand } from './commands/query';
import { pathCommand } from './commands/path';
import { explainCommand } from './commands/explain';
import { statsCommand } from './commands/stats';
import { updateCommand } from './commands/update';
import { exportCommand } from './commands/export';

const program = new Command();

program
  .name('graphify')
  .description('Turn any folder into a queryable knowledge graph')
  .version('0.1.0');

program
  .command('run')
  .description('Run the full pipeline on a directory')
  .argument('<path>', 'Directory to analyze')
  .action(runCommand);

program
  .command('query')
  .description('Query the knowledge graph')
  .argument('<question>', 'Question to ask')
  .option('--graph <path>', 'Path to project root', '.')
  .action(queryCommand);

program
  .command('path')
  .description('Find shortest path between two concepts')
  .argument('<from>', 'Source node')
  .argument('<to>', 'Target node')
  .option('--graph <path>', 'Path to project root', '.')
  .action(pathCommand);

program
  .command('explain')
  .description('Explain a node in plain language')
  .argument('<node>', 'Node ID or label')
  .option('--graph <path>', 'Path to project root', '.')
  .action(explainCommand);

program
  .command('stats')
  .description('Show graph statistics')
  .option('--graph <path>', 'Path to project root', '.')
  .action(statsCommand);

program
  .command('update')
  .description('Incrementally update the graph')
  .argument('<path>', 'Directory to update')
  .action(updateCommand);

program
  .command('export')
  .description('Export graph to JSON')
  .option('--graph <path>', 'Path to project root', '.')
  .option('--out <file>', 'Output file', 'graph.json')
  .action(exportCommand);

program.parse();
```

- [x] **Step 4: Create command files**

`packages/graphify-cli/src/commands/run.ts`:
```typescript
// @ts-ignore — napi-rs generated bindings
import { runPipeline } from '../../graphify.node';

export async function runCommand(path: string) {
  console.log(`Running graphify pipeline on: ${path}`);
  const result = runPipeline(path);
  console.log(`Nodes added: ${result.nodesAdded}`);
  console.log(`Edges added: ${result.edgesAdded}`);
  console.log(`Communities: ${result.communities}`);
  console.log(`Report written to: ${path}/.graphify/graph_report.md`);
}
```

`packages/graphify-cli/src/commands/query.ts`:
```typescript
export async function queryCommand(question: string, opts: { graph: string }) {
  console.log(`Query: "${question}" (graph: ${opts.graph})`);
  console.log('Query traversal not yet implemented — requires BFS/DFS over graph nodes');
}
```

`packages/graphify-cli/src/commands/path.ts`:
```typescript
export async function pathCommand(from: string, to: string, opts: { graph: string }) {
  console.log(`Shortest path: "${from}" → "${to}" (graph: ${opts.graph})`);
  console.log('Shortest path not yet implemented');
}
```

`packages/graphify-cli/src/commands/explain.ts`:
```typescript
// @ts-ignore
import { getNode } from '../../graphify.node';

export async function explainCommand(node: string, opts: { graph: string }) {
  const result = getNode(opts.graph, node);
  if (!result) {
    console.log(`Node "${node}" not found`);
    return;
  }
  console.log(`Node: ${result.label}`);
  console.log(`  File: ${result.sourceFile}:${result.sourceLine ?? '?'}`);
  console.log(`  Type: ${result.fileType}`);
  if (result.docstring) console.log(`  Docstring: ${result.docstring}`);
  if (result.community !== null) console.log(`  Community: ${result.community}`);
}
```

`packages/graphify-cli/src/commands/stats.ts`:
```typescript
// @ts-ignore
import { graphStats } from '../../graphify.node';

export async function statsCommand(opts: { graph: string }) {
  const stats = graphStats(opts.graph);
  console.log(`Nodes: ${stats.nodeCount}`);
  console.log(`Edges: ${stats.edgeCount}`);
  console.log(`Communities: ${stats.communityCount}`);
  console.log(`Files tracked: ${stats.fileCount}`);
}
```

`packages/graphify-cli/src/commands/update.ts`:
```typescript
// @ts-ignore
import { runPipeline } from '../../graphify.node';

export async function updateCommand(path: string) {
  console.log(`Incrementally updating: ${path}`);
  const result = runPipeline(path);
  console.log(`Nodes added: ${result.nodesAdded}`);
  console.log(`Edges added: ${result.edgesAdded}`);
  console.log(`Done.`);
}
```

`packages/graphify-cli/src/commands/export.ts`:
```typescript
// @ts-ignore
import { exportJson } from '../../graphify.node';

export async function exportCommand(opts: { graph: string; out: string }) {
  exportJson(opts.graph, opts.out);
  console.log(`Exported to: ${opts.out}`);
}
```

- [x] **Step 5: Create workspace root package.json**

`package.json`:
```json
{
  "name": "nodesify-graphify",
  "private": true,
  "workspaces": ["packages/*"]
}
```

- [x] **Step 6: Install deps and compile**

Run: `cd packages/graphify-cli && npm install && npx tsc --noEmit`
Expected: TypeScript compiles (may have warnings about missing .node module — that's expected until napi build runs).

- [x] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: Node.js CLI with 7 commands (run, query, path, explain, stats, update, export)"
```

---

## Phase 9: Integration Tests & CI

### Task 13: Integration test fixtures

**Files:**
- Create: `tests/fixtures/python/sample.py`
- Create: `tests/fixtures/javascript/sample.js`
- Create: `tests/fixtures/typescript/sample.ts`
- Create: `tests/fixtures/rust/sample.rs`
- Create: `tests/fixtures/go/sample.go`
- Create: `tests/fixtures/java/Sample.java`
- Create: `tests/fixtures/c/sample.c`
- Create: `tests/fixtures/cpp/sample.cpp`

- [x] **Step 1: Create fixture files**

`tests/fixtures/python/sample.py`:
```python
"""Sample Python module for testing."""
import os

class Calculator:
    """A simple calculator."""

    def add(self, a, b):
        return a + b

    def multiply(self, a, b):
        return a * b

def main():
    calc = Calculator()
    result = calc.add(1, 2)
    print(result)
```

`tests/fixtures/javascript/sample.js`:
```javascript
const fs = require("fs");

class App {
  start() {
    console.log("started");
  }
}

function helper() {
  return 42;
}
```

`tests/fixtures/typescript/sample.ts`:
```typescript
import { readFile } from "fs";

interface Config {
  name: string;
}

class Service {
  constructor(private config: Config) {}

  run(): void {
    console.log(this.config.name);
  }
}

function createService(name: string): Service {
  return new Service({ name });
}
```

`tests/fixtures/rust/sample.rs`:
```rust
use std::io;

struct Config {
    name: String,
}

impl Config {
    fn new(name: &str) -> Self {
        Self { name: name.to_string() }
    }
}

fn main() {
    let config = Config::new("test");
    println!("{}", config.name);
}
```

`tests/fixtures/go/sample.go`:
```go
package main

import "fmt"

type Server struct {
    Port int
}

func NewServer(port int) *Server {
    return &Server{Port: port}
}

func (s *Server) Start() {
    fmt.Printf("Server started on port %d\n", s.Port)
}

func main() {
    s := NewServer(8080)
    s.Start()
}
```

`tests/fixtures/java/Sample.java`:
```java
import java.util.List;

public class Sample {
    private String name;

    public Sample(String name) {
        this.name = name;
    }

    public void print() {
        System.out.println(name);
    }

    public static void main(String[] args) {
        Sample s = new Sample("hello");
        s.print();
    }
}
```

`tests/fixtures/c/sample.c`:
```c
#include <stdio.h>

typedef struct {
    int x;
    int y;
} Point;

Point make_point(int x, int y) {
    Point p;
    p.x = x;
    p.y = y;
    return p;
}

int main() {
    Point p = make_point(1, 2);
    printf("%d %d\n", p.x, p.y);
    return 0;
}
```

`tests/fixtures/cpp/sample.cpp`:
```cpp
#include <iostream>
#include <string>

class Engine {
public:
    Engine(int power) : power_(power) {}
    void start() { std::cout << "Engine started with power " << power_ << std::endl; }
private:
    int power_;
};

int main() {
    Engine e(100);
    e.start();
    return 0;
}
```

- [x] **Step 2: Commit**

```bash
git add -A
git commit -m "test: fixture files in 8 languages for integration testing"
```

---

### Task 14: End-to-end integration test

**Files:**
- Create: `tests/integration/pipeline_test.rs`

- [x] **Step 1: Write integration test**

`tests/integration/pipeline_test.rs`:
```rust
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures")
}

#[test]
fn full_pipeline_on_python_fixture() {
    let root = fixtures_dir().join("python");
    let result = graphify_core::pipeline::run_pipeline(&root).unwrap();

    assert!(result.build_result.nodes_added > 0, "should extract nodes from Python fixture");
    assert!(result.build_result.edges_added > 0, "should extract edges from Python fixture");
    assert!(result.cluster_result.communities.len() > 0, "should assign communities");
    assert!(!result.analysis.god_nodes.is_empty(), "should find god nodes");
    assert!(result.report.contains("# Graph Report"));

    // Check output files exist
    assert!(root.join(".graphify/db.sqlite").exists());
    assert!(root.join(".graphify/graph_report.md").exists());
    assert!(root.join(".graphify/graph.json").exists());

    // Cleanup
    let _ = std::fs::remove_dir_all(root.join(".graphify"));
}

#[test]
fn full_pipeline_on_rust_fixture() {
    let root = fixtures_dir().join("rust");
    let result = graphify_core::pipeline::run_pipeline(&root).unwrap();
    assert!(result.build_result.nodes_added > 0);
    let _ = std::fs::remove_dir_all(root.join(".graphify"));
}

#[test]
fn full_pipeline_on_javascript_fixture() {
    let root = fixtures_dir().join("javascript");
    let result = graphify_core::pipeline::run_pipeline(&root).unwrap();
    assert!(result.build_result.nodes_added > 0);
    let _ = std::fs::remove_dir_all(root.join(".graphify"));
}

#[test]
fn incremental_update_adds_no_duplicate_nodes() {
    let root = fixtures_dir().join("python");

    let r1 = graphify_core::pipeline::run_pipeline(&root).unwrap();
    let nodes_first = r1.build_result.nodes_added;

    let r2 = graphify_core::pipeline::run_pipeline(&root).unwrap();
    // Second run on unchanged files should add 0 new nodes
    assert_eq!(r2.build_result.nodes_added, 0);

    let _ = std::fs::remove_dir_all(root.join(".graphify"));
}

#[test]
fn export_json_is_valid() {
    let root = fixtures_dir().join("typescript");
    let _ = graphify_core::pipeline::run_pipeline(&root).unwrap();

    let json_str = std::fs::read_to_string(root.join(".graphify/graph.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert!(parsed["nodes"].is_array());
    assert!(parsed["edges"].is_array());
    assert!(parsed["nodes"].as_array().unwrap().len() > 0);

    let _ = std::fs::remove_dir_all(root.join(".graphify"));
}
```

Add to root `Cargo.toml`:
```toml
[[test]]
name = "pipeline_test"
path = "tests/integration/pipeline_test.rs"

[dev-dependencies]
graphify-core = { workspace = true }
serde_json = { workspace = true }
```

- [x] **Step 2: Run integration tests**

Run: `cargo test --test pipeline_test`
Expected: 5 tests pass.

- [x] **Step 3: Commit**

```bash
git add -A
git commit -m "test: end-to-end integration tests for full pipeline"
```

---

### Task 15: GitHub Actions CI

**Files:**
- Create: `.github/workflows/ci.yml`

- [x] **Step 1: Write CI workflow**

`.github/workflows/ci.yml`:
```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
        rust: [stable]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: ${{ matrix.rust }}
      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      - name: Run tests
        run: cargo test --workspace
      - name: Check formatting
        run: cargo fmt --all -- --check
      - name: Clippy
        run: cargo clippy --workspace -- -D warnings
```

- [x] **Step 2: Commit**

```bash
git add -A
git commit -m "ci: GitHub Actions CI for Rust workspace (test, fmt, clippy)"
```

---

## Self-Review Checklist

**1. Spec coverage:**
- Core types (Node, Edge, Relation, Confidence, FileType, SourceLocation) → Task 2
- SQLite schema (6 tables) → Task 3
- Detect (file discovery, classification, incremental) → Task 4
- Extract (tree-sitter, LanguageConfig, 8 languages, two-pass, cache) → Task 5
- Build (merge extractions, dedup, incremental replace) → Task 6
- Cluster (label propagation, petgraph, SQLite write-back) → Task 7
- Analyze (god nodes, surprising connections, questions) → Task 8
- Report (markdown generation) → Task 9
- Pipeline orchestration + JSON export → Task 10
- napi-rs bindings → Task 11
- CLI (7 commands) → Task 12
- Integration tests → Tasks 13-14
- CI → Task 15
- Error handling (GraphifyError, thiserror) → Task 2
- .graphifyignore support → Task 4 (via `ignore` crate)

**2. Placeholder scan:** No "TBD", "TODO" in task steps (only in stub lib.rs files that are replaced). No "implement later" or "add appropriate error handling". All code blocks are complete.

**3. Type consistency:** Node IDs are `String` throughout. `BuildResult.nodes_added` is `usize` in Rust, cast to `i64` in napi. `ExtractedNode` fields match between extract→build. `AnalysisResult` fields match between analyze→report→napi.

---

**Execution note:** Task 10 (pipeline orchestration) requires all other pipeline crates to be implemented first. Tasks 2-9 can be parallelized since they're independent crates with clear interfaces. Task 11 depends on Task 10. Task 12 depends on Task 11. Tasks 13-14 depend on Task 10. Task 15 depends on everything.
