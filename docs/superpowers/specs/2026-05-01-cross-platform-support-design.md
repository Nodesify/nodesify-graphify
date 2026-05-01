# Cross-Platform Support Design

Date: 2026-05-01

## Goal

Ensure nodesify-graphify works correctly on Linux, macOS, and Windows — fixing existing platform bugs, centralizing path handling, and adding CI/release infrastructure for all three platforms.

## Approach

Centralized path module (Approach 2): create a `graphify-paths` crate that owns all path normalization and directory resolution. All other crates depend on it instead of doing ad-hoc `.replace('\\', "/")`. This prevents the class of bugs where a call site forgets to normalize (P1, P2).

Minimum Node.js version: 20.

---

## Section 1: `graphify-paths` crate

New crate at `crates/graphify-paths/`.

### Public API

```rust
/// Normalize a path to forward-slash representation for DB storage.
/// On Windows, converts all backslashes to forward slashes.
/// On Unix, returns the path unchanged (already uses forward slashes).
pub fn normalize(p: &Path) -> String

/// Return the `.graphify` directory path under the given root.
/// Creates the directory if it does not exist.
pub fn graphify_dir(root: &Path) -> Result<PathBuf>

/// Return the SQLite database path: `root/.graphify/db.sqlite`.
pub fn db_path(root: &Path) -> Result<PathBuf>
```

### Dependencies

None beyond `std`. No `rusqlite`, no `napi` — pure path logic.

### Consumers

`graphify-detect`, `graphify-extract`, `graphify-build`, `graphify-napi` each add `graphify-paths` as a dependency and replace their inline `.replace('\\', "/")` calls with `graphify_paths::normalize()`.

`graphify-napi` pipeline code replaces `root.join(".graphify")` and `root.join(".graphify").join("db.sqlite")` with `graphify_paths::graphify_dir()` and `graphify_paths::db_path()`.

---

## Section 2: Path normalization bug fixes

Three bugs fixed by routing through `graphify-paths::normalize()`:

| ID | File | Line(s) | Fix |
|----|------|---------|-----|
| P1 | `crates/graphify-detect/src/lib.rs` | 210 | `DELETE FROM file_manifest WHERE file_path = ?1` — normalize the path parameter before binding |
| P2 | `crates/graphify-extract/src/engine.rs` | 70-71, 96-97 | Extraction cache key — normalize before using as cache lookup key |
| — | `crates/graphify-detect/src/lib.rs` | 113, 121, 200 | Replace inline `.replace('\\', "/")` with `normalize()` |
| — | `crates/graphify-build/src/lib.rs` | 26, 123 | Replace inline `.replace('\\', "/")` with `normalize()` |
| — | `crates/graphify-extract/src/engine.rs` | 20 | Replace inline `.replace('\\', "/")` with `normalize()` |
| — | `crates/graphify-napi/src/pipeline.rs` | 57 | Replace inline `.replace('\\', "/")` with `normalize()` |

No logic changes — same behavior, single source of truth.

---

## Section 3: Git hooks rewritten as JavaScript

**File:** `packages/graphify-cli/src/install/hooks.ts`

Current hooks are `/bin/sh` scripts that fail on Windows without Git Bash. Rewrite as JS:

### Changes

- `POST_COMMIT_SCRIPT` and `POST_CHECKOUT_SCRIPT` become JS strings using `child_process.execSync` to invoke `nodesify-graphify update`
- Shebang: `#!/usr/bin/env node` (cross-platform)
- Path construction uses `path.join()` instead of Unix string concatenation
- The `chmod` catch block remains (correct for Windows NTFS)
- Uninstall detection checks for both `#!/usr/bin/env node` and `#!/bin/sh` shebangs for backward compatibility with existing installations

### Example hook structure

```js
#!/usr/bin/env node
const { execSync } = require('child_process');
const path = require('path');
const graphDir = path.join(process.cwd(), '.graphify');
try {
  if (require('fs').existsSync(graphDir)) {
    execSync('nodesify-graphify update .', { stdio: 'inherit' });
  }
} catch {}
```

---

## Section 4: CI hardening

**File:** `.github/workflows/ci.yml`

### Changes

1. **Replace manual cache with `Swatinem/rust-cache@v2`** — handles platform-specific Cargo paths automatically (no more `~/.cargo/...` that breaks on Windows)
2. **Add `@napi-rs/cli` to `devDependencies`** in `packages/graphify-cli/package.json`
3. **Add npm build + test step** after Rust tests:
   ```yaml
   - run: cd packages/graphify-cli && npm install && npm run build && npm test
   ```
4. **Add `engines` field** to `packages/graphify-cli/package.json`:
   ```json
   "engines": { "node": ">=20" }
   ```

---

## Section 5: Cross-platform release workflow

New file: `.github/workflows/release.yml`

### Structure

- **Trigger:** tags matching `v*`
- **Matrix:**
  - `ubuntu-latest` (x86_64 gnu)
  - `windows-latest` (x86_64 msvc)
  - `macos-latest` (aarch64)
  - `macos-13` (x86_64 — last Intel macOS runner)
- **Steps:**
  1. Checkout
  2. Install Rust toolchain
  3. Build napi artifacts via `napi-rs/napi-rs/action/build@main`
  4. Upload artifacts
- **Release job:** downloads all artifacts, runs `npm publish`

---

## Section 6: Watch command Windows fixes

**File:** `packages/graphify-cli/src/commands/watch.ts`

### Changes

1. **SIGINT on Windows** — use the `readline` module to detect Ctrl+C:
   ```ts
   if (process.platform === 'win32') {
     const rl = require('readline').createInterface({ input: process.stdin });
     rl.on('SIGINT', () => { watcher.close(); console.log('Stopped.'); process.exit(0); });
   }
   ```
2. **SIGTERM handler** for Unix `kill` support:
   ```ts
   process.on('SIGTERM', () => { watcher.close(); console.log('Stopped.'); process.exit(0); });
   ```
3. Node >= 20 guarantees `fs.watch` recursive support on Linux — documented via `engines` field.

---

## Section 7: Console output path normalization

**Files:** `run.ts`, `update.ts`, `cluster.ts`, `merge.ts` in `packages/graphify-cli/src/commands/`

Replace hardcoded `${path}/.graphify/...` with `path.join(path, '.graphify', '...')`:

| File | Current | Fixed |
|------|---------|-------|
| `run.ts` | `` `${path}/.graphify/graph_report.md` `` | `path.join(path, '.graphify', 'graph_report.md')` |
| `update.ts` | `` `${path}/.graphify/graph_report.md` `` | `path.join(path, '.graphify', 'graph_report.md')` |
| `cluster.ts` | similar pattern | `path.join(...)` |
| `merge.ts` | similar pattern | `path.join(...)` |

Ensures clean, platform-appropriate path separators in user-facing output.

---

## Testing strategy

- All existing `cargo test` must pass on all three platforms
- New unit tests in `graphify-paths` for `normalize()` covering: Windows backslash paths, UNC paths, mixed separators, already-normalized Unix paths
- Integration test for git hooks: install + uninstall on each platform
- CI matrix covers Linux, Windows, macOS
