# Cross-Platform Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make nodesify-graphify work correctly on Linux, macOS, and Windows — fix existing bugs, centralize path handling, and add CI/release infrastructure.

**Architecture:** Create a `graphify-paths` crate as a single source of truth for path normalization and `.graphify` directory resolution. All other crates consume it. Rewrite git hooks as JS. Harden CI for all three platforms. Add cross-platform release workflow.

**Tech Stack:** Rust (new crate), TypeScript (hooks, watch, CLI), GitHub Actions (CI/release), napi-rs (cross-platform builds)

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Create | `crates/graphify-paths/Cargo.toml` | Crate manifest |
| Create | `crates/graphify-paths/src/lib.rs` | `normalize()`, `graphify_dir()`, `db_path()` |
| Modify | `Cargo.toml` (workspace root) | Add `graphify-paths` to workspace members and dependencies |
| Modify | `crates/graphify-detect/Cargo.toml` | Add `graphify-paths` dependency |
| Modify | `crates/graphify-detect/src/lib.rs` | Use `graphify_paths::normalize()` |
| Modify | `crates/graphify-extract/Cargo.toml` | Add `graphify-paths` dependency |
| Modify | `crates/graphify-extract/src/engine.rs` | Use `graphify_paths::normalize()` |
| Modify | `crates/graphify-build/Cargo.toml` | Add `graphify-paths` dependency |
| Modify | `crates/graphify-build/src/lib.rs` | Use `graphify_paths::normalize()`, remove local `normalize_path()` |
| Modify | `crates/graphify-napi/Cargo.toml` | Add `graphify-paths` dependency |
| Modify | `crates/graphify-napi/src/pipeline.rs` | Use `graphify_paths::normalize()`, `graphify_dir()`, `db_path()` |
| Modify | `packages/graphify-cli/src/install/hooks.ts` | Rewrite hooks as JS |
| Modify | `packages/graphify-cli/src/commands/watch.ts` | Windows SIGINT fix, SIGTERM handler |
| Modify | `packages/graphify-cli/src/commands/run.ts` | Normalize console output paths |
| Modify | `packages/graphify-cli/src/commands/update.ts` | Normalize console output paths |
| Modify | `packages/graphify-cli/src/commands/cluster.ts` | Normalize console output paths |
| Modify | `packages/graphify-cli/src/commands/merge.ts` | Normalize console output paths |
| Modify | `packages/graphify-cli/package.json` | Add `engines`, add `@napi-rs/cli` devDep |
| Modify | `.github/workflows/ci.yml` | Rust-cache, npm test step |
| Create | `.github/workflows/release.yml` | Cross-platform napi build + npm publish |

---

### Task 1: Create `graphify-paths` crate

**Files:**
- Create: `crates/graphify-paths/Cargo.toml`
- Create: `crates/graphify-paths/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Write tests for the path functions**

```rust
// crates/graphify-paths/src/lib.rs (at the bottom)
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn normalize_unix_path_unchanged() {
        assert_eq!(normalize(Path::new("src/main.rs")), "src/main.rs");
    }

    #[test]
    fn normalize_converts_backslashes() {
        assert_eq!(normalize(Path::new("src\\main.rs")), "src/main.rs");
    }

    #[test]
    fn normalize_mixed_separators() {
        assert_eq!(normalize(Path::new("src\\lib/mod.rs")), "src/lib/mod.rs");
    }

    #[test]
    fn normalize_already_normalized() {
        assert_eq!(normalize(Path::new("a/b/c.py")), "a/b/c.py");
    }

    #[test]
    fn graphify_dir_creates_directory() {
        let dir = tempfile::tempdir().unwrap();
        let gf = graphify_dir(dir.path()).unwrap();
        assert!(gf.exists());
        assert!(gf.to_string_lossy().contains(".graphify"));
    }

    #[test]
    fn db_path_is_inside_graphify_dir() {
        let dir = tempfile::tempdir().unwrap();
        let db = db_path(dir.path()).unwrap();
        assert!(db.to_string_lossy().contains(".graphify"));
        assert!(db.to_string_lossy().contains("db.sqlite"));
    }
}
```

- [ ] **Step 2: Create the crate manifest**

```toml
# crates/graphify-paths/Cargo.toml
[package]
name = "graphify-paths"
version = "0.1.0"
edition = "2021"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Write the implementation**

```rust
// crates/graphify-paths/src/lib.rs
use std::path::{Path, PathBuf};

/// Normalize a path to forward-slash representation for DB storage.
/// Converts all backslashes to forward slashes on any platform.
pub fn normalize(p: &Path) -> String {
    p.to_string_lossy().to_string().replace('\\', "/")
}

/// Return the `.graphify` directory path under the given root.
/// Creates the directory if it does not exist.
pub fn graphify_dir(root: &Path) -> std::io::Result<PathBuf> {
    let dir = root.join(".graphify");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Return the SQLite database path: `root/.graphify/db.sqlite`.
/// Creates the `.graphify` directory if it does not exist.
pub fn db_path(root: &Path) -> std::io::Result<PathBuf> {
    let dir = graphify_dir(root)?;
    Ok(dir.join("db.sqlite"))
}
```

- [ ] **Step 4: Register in workspace root `Cargo.toml`**

Add `graphify-paths` to the `members` array (after `graphify-core`) and add it to `[workspace.dependencies]`:

```toml
# In the members array, add:
    "crates/graphify-paths",

# In [workspace.dependencies], add:
graphify-paths = { path = "crates/graphify-paths" }
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p graphify-paths`
Expected: All 6 tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/graphify-paths/ Cargo.toml
git commit -m "feat: add graphify-paths crate for cross-platform path handling"
```

---

### Task 2: Wire `graphify-paths` into `graphify-detect`

**Files:**
- Modify: `crates/graphify-detect/Cargo.toml`
- Modify: `crates/graphify-detect/src/lib.rs`

- [ ] **Step 1: Add dependency to `crates/graphify-detect/Cargo.toml`**

Add under `[dependencies]`:
```toml
graphify-paths.workspace = true
```

- [ ] **Step 2: Replace inline normalization in `crates/graphify-detect/src/lib.rs`**

Add import at top:
```rust
use graphify_paths::normalize;
```

Replace line 113:
```rust
// Before:
let rel_str = relative.to_string_lossy().to_string().replace('\\', "/");
// After:
let rel_str = normalize(relative);
```

Replace line 121:
```rust
// Before:
let rel_str = relative.to_string_lossy().to_string().replace('\\', "/");
// After:
let rel_str = normalize(relative);
```

Replace line 200 (inside `update_manifest`):
```rust
// Before:
entry.path.to_string_lossy().to_string().replace('\\', "/"),
// After:
normalize(&entry.path),
```

Replace line 210 (the DELETE bug — P1 fix):
```rust
// Before:
db.execute("DELETE FROM file_manifest WHERE file_path = ?1", rusqlite::params![entry.path.to_string_lossy().to_string()])?;
// After:
db.execute("DELETE FROM file_manifest WHERE file_path = ?1", rusqlite::params![normalize(&entry.path)])?;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p graphify-detect`
Expected: All existing tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/graphify-detect/
git commit -m "fix: use graphify-paths in graphify-detect (P1 path normalization bug)"
```

---

### Task 3: Wire `graphify-paths` into `graphify-extract`

**Files:**
- Modify: `crates/graphify-extract/Cargo.toml`
- Modify: `crates/graphify-extract/src/engine.rs`

- [ ] **Step 1: Add dependency to `crates/graphify-extract/Cargo.toml`**

Add under `[dependencies]`:
```toml
graphify-paths.workspace = true
```

- [ ] **Step 2: Fix cache key normalization in `crates/graphify-extract/src/engine.rs`**

Add import at top:
```rust
use graphify_paths::normalize;
```

Replace in `check_cache` function (line 70):
```rust
// Before:
let path_str = path.to_string_lossy();
// After:
let path_str = normalize(path);
```

Update line 76 to use a String reference:
```rust
// Before:
rusqlite::params![path_str.as_ref(), hash],
// After:
rusqlite::params![path_str, hash],
```

Replace in `save_cache` function (line 97):
```rust
// Before:
let path_str = path.to_string_lossy();
// After:
let path_str = normalize(path);
```

Update line 105 to use a String reference:
```rust
// Before:
rusqlite::params![path_str.as_ref(), hash, ...]
// After:
rusqlite::params![path_str, hash, ...]
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p graphify-extract`
Expected: All existing tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/graphify-extract/
git commit -m "fix: use graphify-paths in graphify-extract (P2 cache key normalization bug)"
```

---

### Task 4: Wire `graphify-paths` into `graphify-build`

**Files:**
- Modify: `crates/graphify-build/Cargo.toml`
- Modify: `crates/graphify-build/src/lib.rs`

- [ ] **Step 1: Add dependency to `crates/graphify-build/Cargo.toml`**

Add under `[dependencies]`:
```toml
graphify-paths.workspace = true
```

- [ ] **Step 2: Replace local `normalize_path` with `graphify_paths::normalize` in `crates/graphify-build/src/lib.rs`**

Add import at top:
```rust
use graphify_paths::normalize;
```

Replace all `normalize_path(&...)` calls with `normalize(&...)`:
- Line 26: `normalize(&extraction.file_path)` instead of `extraction.file_path.to_string_lossy().to_string().replace('\\', "/")`
- Line 64: `normalize(&node.source_file)` instead of `normalize_path(&node.source_file)`
- Line 74: `normalize(&edge.source_file)` instead of `normalize_path(&edge.source_file)`
- Line 76: `normalize(&edge.source_file)` instead of `normalize_path(&edge.source_file)`
- Line 87: `normalize(&edge.source_file)` instead of `normalize_path(&edge.source_file)`

Delete the local `normalize_path` function (lines 122-124):
```rust
// DELETE this function:
fn normalize_path(path: &std::path::Path) -> String {
    path.to_string_lossy().to_string().replace('\\', "/")
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p graphify-build`
Expected: All existing tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/graphify-build/
git commit -m "refactor: use graphify-paths in graphify-build, remove local normalize_path"
```

---

### Task 5: Wire `graphify-paths` into `graphify-napi`

**Files:**
- Modify: `crates/graphify-napi/Cargo.toml`
- Modify: `crates/graphify-napi/src/pipeline.rs`

- [ ] **Step 1: Add dependency to `crates/graphify-napi/Cargo.toml`**

Add under `[dependencies]`:
```toml
graphify-paths.workspace = true
```

- [ ] **Step 2: Update `crates/graphify-napi/src/pipeline.rs`**

Add import at top:
```rust
use graphify_paths::{self, normalize};
```

In `run_pipeline`, replace lines 23-27:
```rust
// Before:
let graphify_dir = root.join(".graphify");
std::fs::create_dir_all(&graphify_dir)?;

let db_path = graphify_dir.join("db.sqlite");
let db = db::open_db(&db_path)?;

// After:
let graphify_dir = graphify_paths::graphify_dir(root)?;
let db_path = graphify_paths::db_path(root)?;
let db = db::open_db(&db_path)?;
```

In `run_pipeline_inner`, replace line 57:
```rust
// Before:
let path_str = entry.path.to_string_lossy().to_string().replace('\\', "/");
// After:
let path_str = normalize(&entry.path);
```

In `load_graph_db`, replace line 180:
```rust
// Before:
db::open_db(&root.join(".graphify").join("db.sqlite"))

// After:
let p = graphify_paths::db_path(root)?;
db::open_db(&p)
```

Note: `load_graph_db` return type is `graphify_core::Result<Connection>`. Since `db_path` returns `std::io::Result`, convert with `?` which works because `GraphifyError` implements `From<std::io::Error>`.

- [ ] **Step 3: Run tests**

Run: `cargo test -p graphify-napi`
Expected: All existing tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/graphify-napi/
git commit -m "refactor: use graphify-paths in graphify-napi pipeline"
```

---

### Task 6: Run full workspace test

- [ ] **Step 1: Run all tests**

Run: `cargo test --workspace`
Expected: All tests PASS across all crates

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace -- -D warnings`
Expected: No warnings

- [ ] **Step 3: Commit if any fixes were needed**

```bash
git commit -m "fix: address clippy warnings from graphify-paths integration"
```

---

### Task 7: Rewrite git hooks as JavaScript

**Files:**
- Modify: `packages/graphify-cli/src/install/hooks.ts`

- [ ] **Step 1: Rewrite `hooks.ts` with JS-based hooks**

Replace the entire file content:

```typescript
import * as fs from 'fs';
import * as path from 'path';
import { execSync } from 'child_process';

const POST_COMMIT_SCRIPT = `// nodesify-graphify-hook-start
const { execSync, existsSync } = require('child_process');
const { existsSync: existsSyncPath } = require('fs');
const path = require('path');

try {
  const gitDir = execSync('git rev-parse --git-dir 2>/dev/null', { encoding: 'utf-8' }).trim();
  const checks = [
    path.join(gitDir, 'rebase-merge'),
    path.join(gitDir, 'rebase-apply'),
    path.join(gitDir, 'MERGE_HEAD'),
    path.join(gitDir, 'CHERRY_PICK_HEAD'),
  ];
  if (checks.some(p => existsSyncPath(p))) process.exit(0);

  const changed = execSync('git diff --name-only HEAD~1 HEAD 2>/dev/null || git diff --name-only HEAD 2>/dev/null', { encoding: 'utf-8' }).trim();
  if (!changed) process.exit(0);

  const codeExts = new Set(['.py', '.js', '.ts', '.tsx', '.jsx', '.rs', '.go', '.java', '.c', '.h', '.cpp', '.cc', '.cxx', '.hpp']);
  const hasCode = changed.split(/\\r?\\n/).some(f => codeExts.has(path.extname(f)));
  if (hasCode && existsSyncPath('.graphify')) {
    execSync('nodesify-graphify update .', { stdio: 'inherit' });
  }
} catch {}
// nodesify-graphify-hook-end
`;

const POST_CHECKOUT_SCRIPT = `// nodesify-graphify-checkout-hook-start
const { execSync } = require('child_process');
const { existsSync } = require('fs');
const path = require('path');

const branchSwitch = process.argv[3];
if (branchSwitch !== '1') process.exit(0);
if (!existsSync('.graphify')) process.exit(0);

try {
  const gitDir = execSync('git rev-parse --git-dir 2>/dev/null', { encoding: 'utf-8' }).trim();
  if (existsSync(path.join(gitDir, 'rebase-merge')) || existsSync(path.join(gitDir, 'rebase-apply'))) process.exit(0);

  console.log('[nodesify-graphify] Branch switched - rebuilding knowledge graph...');
  execSync('nodesify-graphify update .', { stdio: 'inherit' });
} catch {}
// nodesify-graphify-checkout-hook-end
`;

function getGitRoot(projectDir: string): string | null {
  try {
    const result = execSync('git rev-parse --show-toplevel', {
      cwd: projectDir,
      encoding: 'utf-8',
    }).trim();
    return result;
  } catch {
    return null;
  }
}

function getHooksDir(gitRoot: string): string {
  try {
    const customPath = execSync('git config core.hooksPath', {
      cwd: gitRoot,
      encoding: 'utf-8',
    }).trim();
    if (customPath) {
      return path.isAbsolute(customPath) ? customPath : path.join(gitRoot, customPath);
    }
  } catch {
    // no custom hooks path
  }
  return path.join(gitRoot, '.git', 'hooks');
}

function installHook(hooksDir: string, hookName: string, script: string, startMarker: string, endMarker: string): string {
  if (!fs.existsSync(hooksDir)) {
    fs.mkdirSync(hooksDir, { recursive: true });
  }

  const hookPath = path.join(hooksDir, hookName);

  if (fs.existsSync(hookPath)) {
    const existing = fs.readFileSync(hookPath, 'utf-8');
    if (existing.includes(startMarker)) {
      return `${hookName}: already installed`;
    }
    const appended = existing.trimEnd() + '\n\n' + script;
    fs.writeFileSync(hookPath, appended, 'utf-8');
    return `${hookName}: appended to existing hook`;
  }

  fs.writeFileSync(hookPath, '#!/usr/bin/env node\n\n' + script, 'utf-8');
  try { fs.chmodSync(hookPath, 0o755); } catch { /* Windows */ }
  return `${hookName}: installed`;
}

function uninstallHook(hooksDir: string, hookName: string, startMarker: string, endMarker: string): string {
  const hookPath = path.join(hooksDir, hookName);
  if (!fs.existsSync(hookPath)) {
    return `${hookName}: not found`;
  }

  let content = fs.readFileSync(hookPath, 'utf-8');
  if (!content.includes(startMarker)) {
    return `${hookName}: not installed`;
  }

  const regex = new RegExp(
    '\\n*' + startMarker.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') +
    '[\\s\\S]*?' +
    endMarker.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '\\n*',
    'g'
  );
  content = content.replace(regex, '\n');

  const trimmed = content.trim();
  if (trimmed === '' || trimmed === '#!/bin/sh' || trimmed === '#!/usr/bin/env node') {
    fs.unlinkSync(hookPath);
    return `${hookName}: removed (deleted empty hook)`;
  }

  fs.writeFileSync(hookPath, content, 'utf-8');
  return `${hookName}: removed`;
}

export function installGitHooks(projectDir: string): string[] {
  const gitRoot = getGitRoot(projectDir);
  if (!gitRoot) return ['Not a git repository'];

  const hooksDir = getHooksDir(gitRoot);
  return [
    installHook(hooksDir, 'post-commit', POST_COMMIT_SCRIPT, '// nodesify-graphify-hook-start', '// nodesify-graphify-hook-end'),
    installHook(hooksDir, 'post-checkout', POST_CHECKOUT_SCRIPT, '// nodesify-graphify-checkout-hook-start', '// nodesify-graphify-checkout-hook-end'),
  ];
}

export function uninstallGitHooks(projectDir: string): string[] {
  const gitRoot = getGitRoot(projectDir);
  if (!gitRoot) return ['Not a git repository'];

  const hooksDir = getHooksDir(gitRoot);
  return [
    uninstallHook(hooksDir, 'post-commit', '// nodesify-graphify-hook-start', '// nodesify-graphify-hook-end'),
    uninstallHook(hooksDir, 'post-checkout', '// nodesify-graphify-checkout-hook-start', '// nodesify-graphify-checkout-hook-end'),
  ];
}

export function statusGitHooks(projectDir: string): string[] {
  const gitRoot = getGitRoot(projectDir);
  if (!gitRoot) return ['Not a git repository'];

  const hooksDir = getHooksDir(gitRoot);
  const results: string[] = [];

  for (const [name, marker] of [
    ['post-commit', '// nodesify-graphify-hook-start'],
    ['post-checkout', '// nodesify-graphify-checkout-hook-start'],
  ] as [string, string][]) {
    const hookPath = path.join(hooksDir, name);
    if (fs.existsSync(hookPath)) {
      const content = fs.readFileSync(hookPath, 'utf-8');
      results.push(content.includes(marker) ? `${name}: installed` : `${name}: not installed`);
    } else {
      results.push(`${name}: not installed`);
    }
  }

  return results;
}
```

Key changes:
- `POST_COMMIT_SCRIPT` and `POST_CHECKOUT_SCRIPT` are now JS code, not `/bin/sh`
- Shebang is `#!/usr/bin/env node` instead of `#!/bin/sh`
- Markers changed from `# nodesify-graphify-hook-start` to `// nodesify-graphify-hook-start`
- Uninstall detects both old `#!/bin/sh` and new `#!/usr/bin/env node` empty hooks for backward compat
- All path construction uses `path.join()` instead of string concatenation
- Status checks use `//` markers

- [ ] **Step 2: Commit**

```bash
git add packages/graphify-cli/src/install/hooks.ts
git commit -m "feat: rewrite git hooks as JS for cross-platform support (Windows)"
```

---

### Task 8: Fix watch command for Windows

**Files:**
- Modify: `packages/graphify-cli/src/commands/watch.ts`

- [ ] **Step 1: Update watch.ts with Windows SIGINT and SIGTERM handlers**

Replace lines 47-53 (the SIGINT handler and surrounding console output):

```typescript
  console.log(`[nodesify-graphify] Watching ${resolved} (debounce: ${debounceMs}ms)`);
  console.log('[nodesify-graphify] Press Ctrl+C to stop');

  // Windows SIGINT workaround: readline detects Ctrl+C in Windows terminals
  if (process.platform === 'win32') {
    const readline = require('readline');
    const rl = readline.createInterface({ input: process.stdin });
    rl.on('SIGINT', () => {
      console.log('\n[nodesify-graphify] Stopped.');
      watcher.close();
      rl.close();
      process.exit(0);
    });
  } else {
    process.on('SIGINT', () => {
      console.log('\n[nodesify-graphify] Stopped.');
      watcher.close();
      process.exit(0);
    });
    process.on('SIGTERM', () => {
      console.log('\n[nodesify-graphify] Stopped.');
      watcher.close();
      process.exit(0);
    });
  }
```

- [ ] **Step 2: Commit**

```bash
git add packages/graphify-cli/src/commands/watch.ts
git commit -m "fix: add Windows SIGINT handler and SIGTERM support for watch command"
```

---

### Task 9: Normalize console output paths in CLI commands

**Files:**
- Modify: `packages/graphify-cli/src/commands/run.ts`
- Modify: `packages/graphify-cli/src/commands/update.ts`
- Modify: `packages/graphify-cli/src/commands/cluster.ts`
- Modify: `packages/graphify-cli/src/commands/merge.ts`

- [ ] **Step 1: Fix `run.ts`**

Find:
```typescript
console.log(`Report written to: ${path}/.graphify/graph_report.md`);
```

Replace with:
```typescript
console.log(`Report written to: ${require('path').join(path, '.graphify', 'graph_report.md')}`);
```

Note: these files already import `path` at the top. Use the existing import:
```typescript
console.log(`Report written to: ${path.join(p, '.graphify', 'graph_report.md')}`);
```

Where `p` is the variable name used for the path argument in that file (check the actual variable name — in `run.ts` the parameter is `path`, so rename to avoid shadowing: use `path.join(dirPath, '.graphify', 'graph_report.md')` or import as a different name).

Actually — the variable is named `path` which shadows the `path` module import. Let me check each file's actual import. Read each file to confirm the variable name.

In `run.ts`: the parameter is likely named differently. Let me read the actual file.

For safety, use this approach in all four files — at the top, ensure `path` module is imported, and use `path.join(...)`:

**`run.ts`** — parameter `path` shadows the `path` module. Add import alias:
```typescript
import * as pathMod from 'path';
// @ts-ignore
import { runPipeline } from '../../graphify.node';

export async function runCommand(path: string) {
  console.log(`Running graphify pipeline on: ${path}`);
  const result = runPipeline(path);
  console.log(`Nodes added: ${result.nodesAdded}`);
  console.log(`Edges added: ${result.edgesAdded}`);
  console.log(`Communities: ${result.communities}`);
  console.log(`Report written to: ${pathMod.join(path, '.graphify', 'graph_report.md')}`);
}
```

- [ ] **Step 2: Fix `update.ts`** — same pattern:
```typescript
import * as pathMod from 'path';
// @ts-ignore
import { updatePipeline } from '../../graphify.node';

export async function updateCommand(path: string) {
  console.log(`Running incremental rebuild on: ${path}`);
  const result = updatePipeline(path);
  console.log(`Nodes: ${result.nodesAdded}, Edges: ${result.edgesAdded}, Communities: ${result.communities}`);
  console.log(`Report updated at: ${pathMod.join(path, '.graphify', 'graph_report.md')}`);
}
```

- [ ] **Step 3: Fix `cluster.ts`** — same pattern:
```typescript
import * as pathMod from 'path';
// @ts-ignore
import { clusterOnly } from '../../graphify.node';

export async function clusterCommand(path: string) {
  console.log(`Running cluster + analyze on: ${path}`);
  const result = clusterOnly(path);
  console.log(`Communities: ${result.communities}`);
  console.log(`Report updated at: ${pathMod.join(path, '.graphify', 'graph_report.md')}`);
}
```

- [ ] **Step 4: Fix `merge.ts`** — uses `outPath` not `path`, so no shadowing:
```typescript
import * as path from 'path';
// @ts-ignore
import { mergeGraphs } from '../../graphify.node';

export async function mergeCommand(pathA: string, pathB: string, outPath: string) {
  console.log(`Merging graphs: ${pathA} + ${pathB} -> ${outPath}`);
  const result = mergeGraphs(pathA, pathB, outPath);
  console.log(`Nodes: ${result.nodesAdded}, Edges: ${result.edgesAdded}, Communities: ${result.communities}`);
  console.log(`Merged graph written to: ${path.join(outPath, '.graphify')}`);
}
```

- [ ] **Step 5: Commit**

```bash
git add packages/graphify-cli/src/commands/run.ts packages/graphify-cli/src/commands/update.ts packages/graphify-cli/src/commands/cluster.ts packages/graphify-cli/src/commands/merge.ts
git commit -m "fix: normalize console output paths for cross-platform display"
```

---

### Task 10: Update `package.json` — engines and devDependencies

**Files:**
- Modify: `packages/graphify-cli/package.json`

- [ ] **Step 1: Add `engines` field and `@napi-rs/cli` devDependency**

Add to the root of `package.json` (after `"devDependencies"`):
```json
"engines": {
  "node": ">=20"
}
```

Add to `devDependencies`:
```json
"@napi-rs/cli": "^2"
```

- [ ] **Step 2: Commit**

```bash
git add packages/graphify-cli/package.json
git commit -m "feat: add Node >=20 engines field and @napi-rs/cli devDep"
```

---

### Task 11: Harden CI workflow

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Replace manual cache with `Swatinem/rust-cache`, add npm test step**

Replace the entire file:

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
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with:
          key: ${{ hashFiles('**/Cargo.lock') }}
      - name: Run tests
        run: cargo test --workspace
      - name: Check formatting
        run: cargo fmt --all -- --check
      - name: Clippy
        run: cargo clippy --workspace -- -D warnings

  cli-test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    needs: test
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with:
          key: ${{ hashFiles('**/Cargo.lock') }}
      - name: Build native module
        run: cd packages/graphify-cli && npm install && npm run napi:build
      - name: Build CLI
        run: cd packages/graphify-cli && npm run build
      - name: Run CLI tests
        run: cd packages/graphify-cli && npm test
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: use rust-cache action, add CLI test job across all platforms"
```

---

### Task 12: Add cross-platform release workflow

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Create release workflow**

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: windows-latest
            target: x86_64-pc-windows-msvc
          - os: macos-latest
            target: aarch64-apple-darwin
          - os: macos-13
            target: x86_64-apple-darwin
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - uses: Swatinem/rust-cache@v2
      - name: Build napi artifacts
        uses: napi-rs/napi-rs/action/build@main
        with:
          target: ${{ matrix.target }}
      - uses: actions/upload-artifact@v4
        with:
          name: graphify-${{ matrix.target }}
          path: packages/graphify-cli/*.node

  publish:
    runs-on: ubuntu-latest
    needs: build
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
          registry-url: 'https://registry.npmjs.org'
      - uses: actions/download-artifact@v4
        with:
          path: artifacts
          pattern: 'graphify-*'
      - name: Move artifacts
        run: |
          mkdir -p packages/graphify-cli/napi
          find artifacts -name '*.node' -exec mv {} packages/graphify-cli/ \;
      - name: Install dependencies
        run: cd packages/graphify-cli && npm install
      - name: Build CLI
        run: cd packages/graphify-cli && npm run build
      - name: Publish
        run: cd packages/graphify-cli && npx napi prepublish -t npm
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add cross-platform release workflow with napi artifact builds"
```

---

### Task 13: Final verification

- [ ] **Step 1: Run full workspace tests**

Run: `cargo test --workspace`
Expected: All tests PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace -- -D warnings`
Expected: No warnings

- [ ] **Step 3: Verify CLI builds**

Run: `cd packages/graphify-cli && npm install && npm run build`
Expected: Build succeeds

- [ ] **Step 4: Final commit if any fixes needed**
