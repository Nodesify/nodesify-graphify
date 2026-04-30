# Graphify Rearchitecture Notes

> Observations and improvement opportunities from studying the existing `C:\Nodesify\graphify` (v0.5.6).

---

## Current Pain Points

### 1. `__main__.py` Monolith (~1525 lines)

**Problem**: All CLI command dispatch, argument parsing, and 13 platform install/uninstall functions in a single file. This is the biggest coupling point.

**Impact**: Hard to add new commands or platforms. Any CLI change risks breaking unrelated commands.

**Opportunity**: Split into a command registry pattern — each command in its own module, each platform installer in its own module.

### 2. `extract.py` Oversized (~32K tokens)

**Problem**: 21 language extractors, LanguageConfig dataclass, cross-file resolution, call-graph second pass — all in one file.

**Impact**: Slow to navigate, risky to modify, hard to test individual language extractors in isolation.

**Opportunity**: Extract per-language configs into separate files. Keep the AST-walking engine in `extract.py` but move language definitions to `extractors/` directory.

### 3. `export.py` Mixes Concerns (~1065 lines)

**Problem**: Data preparation, vis.js HTML generation (embedded JS), Obsidian vault writing, and Neo4j push all in one module.

**Impact**: Changing the HTML template risks breaking Neo4j export and vice versa.

**Opportunity**: Split into `exporters/` directory with one module per format.

### 4. No Type Stubs or Protocol Definitions

**Problem**: The extraction schema is enforced by `validate.py` at runtime but there are no `TypedDict`, `Protocol`, or `@dataclass` definitions for the node/edge schemas.

**Impact**: No IDE autocompletion, no static type checking on the core data structures, easy to pass malformed dicts between pipeline stages.

**Opportunity**: Define strict type interfaces for all data that crosses module boundaries.

### 5. Platform Install Logic Entangled in CLI

**Problem**: 13 platform install/uninstall functions with skill file mappings, hook configs, and file path logic all inline in `__main__.py`.

**Impact**: Adding a new platform requires modifying the monolith. Platform-specific quirks are hard to isolate.

**Opportunity**: Extract to a plugin/registry pattern. Each platform provides: install(), uninstall(), skill_file, hook_config.

### 6. Skill File Duplication

**Problem**: 11 nearly-identical skill files (`skill.md`, `skill-aider.md`, etc.) with minor platform-specific differences.

**Impact**: Changes to the core pipeline require updating 11 files.

**Opportunity**: Template-based generation from a single source of truth with platform-specific overlays.

---

## Architectural Strengths to Preserve

### 1. Stateless Pipeline

Each stage is a pure function. No shared state. This is excellent — makes testing, debugging, and parallelization straightforward.

### 2. Clean Separation of AST and LLM Passes

The dual-pass architecture (deterministic AST + semantic LLM) is cleanly separated. They can run independently or swap backends.

### 3. Security-First Design

`security.py` validates all external input. SSRF protection, path traversal prevention, size limits, XSS sanitization — all centralized.

### 4. Comprehensive Test Coverage

22 test files, one per module, pure unit tests. This is a strong foundation.

### 5. Incremental Rebuild Support

`build_merge()` has a safety check that refuses to shrink the graph. Cache split (ast/ vs semantic/) prevents hash collisions.

### 6. Lazy Import Facade

`__init__.py` uses `__getattr__` for lazy imports — fast startup, heavy deps loaded only when needed.

---

## Key Architectural Decisions for Rearchitecture

### Language Choice

The existing graphify is **Python-only**. Key question for rearchitecture:

- **Stay Python?** Preserve tree-sitter bindings, NetworkX, faster-whisper ecosystem.
- **Move to TypeScript/Node.js?** Better alignment with "nodesify" branding, native npm distribution.
- **Hybrid?** TypeScript CLI/interface layer calling Python extraction engine.

### Module Structure Options

**Option A: Flat (current pattern, improved)**
```
src/
  detect.ts
  extract.ts
  build.ts
  cluster.ts
  ...
```

**Option B: Domain-organized**
```
src/
  pipeline/
    detect.ts
    extract.ts
    build.ts
    cluster.ts
    analyze.ts
    report.ts
  exporters/
    json.ts
    html.ts
    obsidian.ts
    neo4j.ts
  extractors/
    python.ts
    javascript.ts
    typescript.ts
    ...
  platforms/
    claude.ts
    codex.ts
    cursor.ts
    ...
  cli/
    commands/
      query.ts
      install.ts
      watch.ts
      ...
```

**Option C: Plugin-based**
```
src/
  core/
    pipeline.ts
    graph.ts
    schema.ts
  plugins/
    extractors/
    exporters/
    platforms/
```

### Graph Engine

Current: NetworkX (Python-only).

Alternatives for TypeScript:
- `graphology` — lightweight, extensive algorithm support
- `@neo4j/graphql` — if Neo4j native
- Custom adjacency list — maximum control, minimal deps

### AST Extraction

Current: tree-sitter Python bindings for 21 languages.

For TypeScript:
- `web-tree-sitter` (WASM-based tree-sitter bindings)
- `tree-sitter` node bindings
- Both support the same 21 languages

---

## Open Questions

1. **Scope**: Are we rewriting from scratch or refactoring the existing Python codebase?
2. **Language**: Python, TypeScript, or hybrid?
3. **Graph engine**: NetworkX equivalent in the target language?
4. **Backwards compatibility**: Must the rearchitected version be CLI-compatible with v0.5.6?
5. **Skill system**: Keep the multi-platform skill approach or simplify?
6. **Distribution**: PyPI, npm, or both?
7. **MCP server**: Keep, drop, or expand?
8. **Incremental migration**: Can we migrate pipeline stages one at a time, or big-bang?
