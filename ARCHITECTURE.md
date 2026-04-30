# Graphify Architecture Reference

> Source: `C:\Nodesify\graphify` (branch `v5`, version 0.5.6, PyPI: `graphifyy`)

This document captures the existing architecture of graphify to inform the rearchitecture of `nodesify-graphify`.

---

## Overview

Graphify converts any folder of code, documents, papers, images, or videos into a queryable knowledge graph. It operates as both a Python CLI tool and a skill for 15+ AI coding assistants.

**Language**: Python >=3.10
**Build system**: setuptools
**Core dependency**: NetworkX (graph storage), tree-sitter (AST parsing)
**No TypeScript, no Node.js** in the current implementation.

---

## Pipeline

```
detect() → extract() → build_graph() → cluster() → analyze() → report() → export()
```

Each stage is a standalone function in its own module. They communicate through plain Python dicts and NetworkX graphs. No shared state, no side effects outside `graphify-out/`.

### Data Flow

```
                    ┌─────────────┐
   Filesystem ────▶ │  detect()   │ ──→ classified file dict with stats
                    └─────────────┘
                           │
                           ▼
                    ┌─────────────┐
   AST pass ──────▶│  extract()  │ ──→ {nodes: [...], edges: [...]}
   LLM pass ──────▶│             │
                    └─────────────┘
                           │
                           ▼
                    ┌─────────────┐
                    │ build_graph │ ──→ nx.Graph
                    └─────────────┘
                           │
                           ▼
                    ┌─────────────┐
                    │  cluster()  │ ──→ nx.Graph with community attrs
                    └─────────────┘
                           │
                           ▼
                    ┌─────────────┐
                    │  analyze()  │ ──→ {god_nodes, surprises, questions}
                    └─────────────┘
                           │
                           ▼
                    ┌─────────────┐
                    │  report()   │ ──→ GRAPH_REPORT.md string
                    └─────────────┘
                           │
                           ▼
                    ┌─────────────┐
                    │  export()   │ ──→ HTML, JSON, SVG, GraphML, Obsidian, Neo4j
                    └─────────────┘
```

---

## Module Responsibilities

### Core Pipeline Modules

| Module | Primary Function | Size | Responsibility |
|--------|-----------------|------|----------------|
| `detect.py` | `detect(root)`, `classify_file()`, `detect_incremental()` | Medium | File discovery, classification (CODE/DOCUMENT/PAPER/IMAGE/VIDEO), `.graphifyignore` support |
| `extract.py` | `extract(paths)` | **Very Large** (~32K tokens) | Tree-sitter AST extraction for 21 languages, cross-file call resolution, rationale extraction |
| `build.py` | `build(extractions)`, `build_from_json()`, `build_merge()` | Medium | Merge extraction dicts into NetworkX graph, safety check against graph shrinkage |
| `cluster.py` | `cluster(G)`, `cohesion_score()` | Small | Leiden/Louvain community detection, graph with `community` attr on nodes |
| `analyze.py` | `god_nodes()`, `surprising_connections()`, `suggest_questions()`, `graph_diff()` | Medium | Graph analysis: high-degree nodes, surprising cross-community connections, suggested questions |
| `report.py` | `generate()` | Medium | GRAPH_REPORT.md generator from graph + analysis |
| `export.py` | `to_json()`, `to_html()`, `to_svg()`, `to_obsidian()`, `to_canvas()`, `to_graphml()`, `to_cypher()`, `push_to_neo4j()` | **Large** (~1065 lines) | Multi-format export with embedded vis.js HTML, Obsidian vault, Neo4j push |

### Supporting Modules

| Module | Function | Responsibility |
|--------|----------|----------------|
| `cache.py` | SHA256 file-level cache, semantic cache | Split AST/semantic caches to prevent hash collisions |
| `security.py` | `validate_url()`, `safe_fetch()`, `sanitize_label()`, `validate_graph_path()` | SSRF protection, size limits, path traversal prevention, XSS sanitization |
| `validate.py` | `validate_extraction(data)` | JSON schema validation for extraction dicts |
| `ingest.py` | `ingest(url)`, `save_query_result()` | URL fetcher (tweets, arxiv, webpages, PDFs) |
| `llm.py` | `extract_files_direct()`, `extract_corpus_parallel()` | Direct LLM backend (Claude, Kimi K2.6) bypassing skill/subagent system |
| `transcribe.py` | `transcribe()`, `transcribe_all()`, `download_audio()` | Video/audio transcription via faster-whisper |
| `serve.py` | `serve(graph_path)` | MCP stdio server exposing 7 graph query tools |
| `watch.py` | `watch(root)`, `_rebuild_code()` | Filesystem watcher via watchdog |
| `benchmark.py` | `run_benchmark()` | Token-reduction benchmarking |
| `wiki.py` | `to_wiki()` | Wikipedia-style markdown export (index.md + community articles) |
| `hooks.py` | `install()`, `uninstall()`, `status()` | Git post-commit/post-checkout hooks |
| `manifest.py` | Re-exports from `detect.py` | Backwards compatibility shim |

### Entry Points

| File | Responsibility |
|------|----------------|
| `__init__.py` | Lazy import facade (avoids loading heavy deps until needed) |
| `__main__.py` | CLI entry point (~1525 lines), all command dispatch |

---

## Data Models

### Extraction Schema (validate.py enforced)

```json
{
  "nodes": [
    {
      "id": "unique_string",
      "label": "human-readable name",
      "file_type": "CODE|DOCUMENT|PAPER|IMAGE|VIDEO",
      "source_file": "path/to/file",
      "source_location": "L42",
      "docstring": "...",
      "rationale": "..."
    }
  ],
  "edges": [
    {
      "source": "id_a",
      "target": "id_b",
      "relation": "calls|imports|uses|defines|...",
      "confidence": "EXTRACTED|INFERRED|AMBIGUOUS",
      "confidence_score": 0.85,
      "source_file": "path/to/file"
    }
  ]
}
```

### Confidence Labels

| Label | Meaning |
|-------|---------|
| `EXTRACTED` | Explicitly stated in source (import, direct call) |
| `INFERRED` | Reasonable deduction (call-graph second pass, co-occurrence) |
| `AMBIGUOUS` | Uncertain; flagged for human review |

### NetworkX Graph Attributes

- **Nodes**: `community`, `label`, `source_file`, `file_type`
- **Edges**: `relation`, `confidence`, `confidence_score`, `_src`, `_tgt`
- **Graph-level**: `G.graph["hyperedges"]` — list of group relationships connecting 3+ nodes

### FileType Enum (detect.py)

`CODE`, `DOCUMENT`, `PAPER`, `IMAGE`, `VIDEO`

### LanguageConfig Dataclass (extract.py)

Declarative config per tree-sitter language specifying:
- Node types for classes, functions, imports, calls
- Name fields and body fields
- Import handlers
- Call-graph walk configuration

---

## CLI Surface

Defined in `__main__.py` (~1525 lines). Entry point: `graphify = "graphify.__main__:main"`.

### Commands

| Command | Purpose |
|---------|---------|
| `graphify install [--platform P]` | Install skill for a specific AI assistant |
| `graphify query "<q>" [--dfs] [--budget N] [--graph path]` | BFS/DFS graph traversal |
| `graphify path "A" "B"` | Shortest path between two concepts |
| `graphify explain "X"` | Plain-language node explanation |
| `graphify add <url> [--author] [--contributor] [--dir]` | Fetch URL into corpus |
| `graphify watch <path>` | Filesystem watcher |
| `graphify update <path>` | AST-only re-extraction |
| `graphify cluster-only <path>` | Re-cluster existing graph |
| `graphify clone <url> [--branch] [--out]` | Clone + prepare repo |
| `graphify merge-graphs <g1> <g2> [--out]` | Cross-repo graph merge |
| `graphify benchmark [graph.json]` | Token reduction benchmark |
| `graphify save-result --question Q --answer A` | Save Q&A to memory |
| `graphify check-update <path>` | Check pending semantic update |
| `graphify hook install/uninstall/status` | Git hook management |
| `graphify claude/codex/gemini/cursor/... install/uninstall` | Per-platform setup |

---

## Skill System

### Skill Pipeline (runs inside AI coding assistant)

1. **Step 0**: Clone GitHub repos if URLs provided
2. **Step 1**: `detect()` — classify and enumerate files
3. **Step 2**: `extract()` — AST extraction via tree-sitter (no LLM)
4. **Step 3**: Transcribe video/audio if present (local Whisper)
5. **Step 4**: Semantic extraction via Claude subagents (parallel over docs/papers/images)
6. **Step 5**: `build()` — assemble NetworkX graph
7. **Step 6**: `cluster()` — Leiden community detection
8. **Step 7**: `analyze()` — god nodes, surprises, questions
9. **Step 8**: `report()` + `export()` — write outputs

### Platform Integration Methods

| Platform | Integration Mechanism |
|----------|----------------------|
| Claude Code | PreToolUse hooks |
| Codex | PreToolUse hooks |
| Gemini CLI | PreToolUse hooks |
| Aider | `AGENTS.md` |
| OpenClaw | `AGENTS.md` |
| Trae | `AGENTS.md` |
| Cursor | `.cursor/rules/graphify.mdc` |
| Kiro | `.kiro/steering/graphify.md` |
| VS Code Copilot | `.github/copilot-instructions.md` |

### Skill Variants

11 platform-specific skill files: `skill.md`, `skill-aider.md`, `skill-claw.md`, `skill-codex.md`, `skill-copilot.md`, `skill-droid.md`, `skill-kiro.md`, `skill-opencode.md`, `skill-trae.md`, `skill-vscode.md`, `skill-windows.md`

---

## MCP Server (serve.py)

Exposes 7 tools over stdio MCP:

| Tool | Purpose |
|------|---------|
| `query_graph` | BFS/DFS traversal with budget |
| `get_node` | Get node details |
| `get_neighbors` | Get node neighbors |
| `get_community` | Get community members |
| `god_nodes` | High-degree hub nodes |
| `graph_stats` | Graph statistics |
| `shortest_path` | Shortest path between nodes |

---

## Security Model

All external input validated through `security.py`:

- **SSRF protection**: `validate_url()` blocks non-http/https, private IPs, cloud metadata
- **DNS rebinding guard**: `_ssrf_guarded_socket()`
- **Size limits**: 50MB binary, 10MB text
- **Path traversal**: `validate_graph_path()` enforces `graphify-out/` containment
- **XSS**: `sanitize_label()` strips control chars, caps 256 chars
- **No network listener**: MCP server is stdio-only
- **No code execution**: tree-sitter parses ASTs without eval/exec

---

## Testing

- **22 test files**, one per module
- **Framework**: pytest
- **Fixtures**: Sample source files in 20 languages under `tests/fixtures/`
- **CI**: GitHub Actions on Python 3.10 + 3.12
- **All pure unit tests** — no network calls, no filesystem side effects outside `tmp_path`

---

## Dependencies

### Core (21 tree-sitter languages + networkx)

`networkx`, `tree-sitter>=0.23.0`, plus 21 language-specific tree-sitter packages.

### Optional Dependency Groups

| Group | Packages |
|-------|----------|
| `mcp` | `mcp` |
| `neo4j` | `neo4j` |
| `pdf` | `pypdf`, `html2text` |
| `watch` | `watchdog` |
| `svg` | `matplotlib` |
| `leiden` | `graspologic` (<3.13) |
| `office` | `python-docx`, `openpyxl` |
| `video` | `faster-whisper`, `yt-dlp` |
| `kimi` | `openai` |

---

## File Structure (Reference)

```
graphify/
├── __init__.py          # Lazy import facade
├── __main__.py          # CLI (~1525 lines)
├── analyze.py           # Graph analysis
├── benchmark.py         # Token reduction benchmarking
├── build.py             # Merge extractions → NetworkX graph
├── cache.py             # SHA256 file-level cache
├── cluster.py           # Leiden/Louvain community detection
├── detect.py            # File discovery + classification
├── export.py            # Multi-format export (~1065 lines)
├── extract.py           # Tree-sitter AST extraction (~32K tokens)
├── hooks.py             # Git hooks
├── ingest.py            # URL fetcher
├── llm.py               # Direct LLM backend
├── manifest.py          # Backwards compat shim
├── report.py            # GRAPH_REPORT.md generator
├── security.py          # Input validation + SSRF guards
├── serve.py             # MCP stdio server (7 tools)
├── transcribe.py        # Whisper video/audio transcription
├── validate.py          # JSON schema validation
├── watch.py             # Filesystem watcher
├── wiki.py              # Wikipedia-style markdown export
├── skill.md             # Claude Code skill definition
└── skill-*.md           # 10 platform-specific skill variants
```
