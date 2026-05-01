"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.installGitHooks = installGitHooks;
exports.uninstallGitHooks = uninstallGitHooks;
exports.statusGitHooks = statusGitHooks;
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
const child_process_1 = require("child_process");
const POST_COMMIT_SCRIPT = `// nodesify-graphify-hook-start
const { execSync } = require('child_process');
const { existsSync } = require('fs');
const path = require('path');

try {
  const gitDir = execSync('git rev-parse --git-dir 2>/dev/null', { encoding: 'utf-8' }).trim();
  const checks = [
    path.join(gitDir, 'rebase-merge'),
    path.join(gitDir, 'rebase-apply'),
    path.join(gitDir, 'MERGE_HEAD'),
    path.join(gitDir, 'CHERRY_PICK_HEAD'),
  ];
  if (checks.some(p => existsSync(p))) process.exit(0);

  const changed = execSync('git diff --name-only HEAD~1 HEAD 2>/dev/null || git diff --name-only HEAD 2>/dev/null', { encoding: 'utf-8' }).trim();
  if (!changed) process.exit(0);

  const codeExts = new Set(['.py', '.js', '.ts', '.tsx', '.jsx', '.rs', '.go', '.java', '.c', '.h', '.cpp', '.cc', '.cxx', '.hpp']);
  const hasCode = changed.split(/\\r?\\n/).some(f => codeExts.has(path.extname(f)));
  if (hasCode && existsSync('.graphify')) {
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
function getGitRoot(projectDir) {
    try {
        const result = (0, child_process_1.execSync)('git rev-parse --show-toplevel', {
            cwd: projectDir,
            encoding: 'utf-8',
        }).trim();
        return result;
    }
    catch {
        return null;
    }
}
function getHooksDir(gitRoot) {
    try {
        const customPath = (0, child_process_1.execSync)('git config core.hooksPath', {
            cwd: gitRoot,
            encoding: 'utf-8',
        }).trim();
        if (customPath) {
            return path.isAbsolute(customPath) ? customPath : path.join(gitRoot, customPath);
        }
    }
    catch {
        // no custom hooks path
    }
    return path.join(gitRoot, '.git', 'hooks');
}
function installHook(hooksDir, hookName, script, startMarker, endMarker) {
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
    try {
        fs.chmodSync(hookPath, 0o755);
    }
    catch { /* Windows */ }
    return `${hookName}: installed`;
}
function uninstallHook(hooksDir, hookName, startMarker, endMarker) {
    const hookPath = path.join(hooksDir, hookName);
    if (!fs.existsSync(hookPath)) {
        return `${hookName}: not found`;
    }
    let content = fs.readFileSync(hookPath, 'utf-8');
    if (!content.includes(startMarker)) {
        return `${hookName}: not installed`;
    }
    const regex = new RegExp('\\n*' + startMarker.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') +
        '[\\s\\S]*?' +
        endMarker.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '\\n*', 'g');
    content = content.replace(regex, '\n');
    const trimmed = content.trim();
    if (trimmed === '' || trimmed === '#!/bin/sh' || trimmed === '#!/usr/bin/env node') {
        fs.unlinkSync(hookPath);
        return `${hookName}: removed (deleted empty hook)`;
    }
    fs.writeFileSync(hookPath, content, 'utf-8');
    return `${hookName}: removed`;
}
function installGitHooks(projectDir) {
    const gitRoot = getGitRoot(projectDir);
    if (!gitRoot)
        return ['Not a git repository'];
    const hooksDir = getHooksDir(gitRoot);
    return [
        installHook(hooksDir, 'post-commit', POST_COMMIT_SCRIPT, '// nodesify-graphify-hook-start', '// nodesify-graphify-hook-end'),
        installHook(hooksDir, 'post-checkout', POST_CHECKOUT_SCRIPT, '// nodesify-graphify-checkout-hook-start', '// nodesify-graphify-checkout-hook-end'),
    ];
}
function uninstallGitHooks(projectDir) {
    const gitRoot = getGitRoot(projectDir);
    if (!gitRoot)
        return ['Not a git repository'];
    const hooksDir = getHooksDir(gitRoot);
    return [
        uninstallHook(hooksDir, 'post-commit', '// nodesify-graphify-hook-start', '// nodesify-graphify-hook-end'),
        uninstallHook(hooksDir, 'post-checkout', '// nodesify-graphify-checkout-hook-start', '// nodesify-graphify-checkout-hook-end'),
    ];
}
function statusGitHooks(projectDir) {
    const gitRoot = getGitRoot(projectDir);
    if (!gitRoot)
        return ['Not a git repository'];
    const hooksDir = getHooksDir(gitRoot);
    const results = [];
    for (const [name, marker] of [
        ['post-commit', '// nodesify-graphify-hook-start'],
        ['post-checkout', '// nodesify-graphify-checkout-hook-start'],
    ]) {
        const hookPath = path.join(hooksDir, name);
        if (fs.existsSync(hookPath)) {
            const content = fs.readFileSync(hookPath, 'utf-8');
            results.push(content.includes(marker) ? `${name}: installed` : `${name}: not installed`);
        }
        else {
            results.push(`${name}: not installed`);
        }
    }
    return results;
}
//# sourceMappingURL=hooks.js.map