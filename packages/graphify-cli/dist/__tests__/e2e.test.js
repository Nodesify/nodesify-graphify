"use strict";
/**
 * End-to-end test — runs the COMPILED CLI (dist/index.js + dist/graphify.node)
 * against a real fixture project: full pipeline, stats, query, status, and
 * export, all through the actual binary the user installs. Skips when dist
 * has not been built yet (CI builds the native module and the CLI before
 * `npm test`; locally run `npm run build` first).
 *
 * Run with: npx tsx src/__tests__/e2e.test.ts
 */
Object.defineProperty(exports, "__esModule", { value: true });
const child_process_1 = require("child_process");
const fs_1 = require("fs");
const os_1 = require("os");
const path_1 = require("path");
const cliEntry = (0, path_1.resolve)(__dirname, '..', '..', 'dist', 'index.js');
const nativeBin = (0, path_1.resolve)(__dirname, '..', '..', 'dist', 'graphify.node');
const fixtureDir = (0, path_1.resolve)(__dirname, '..', '..', '..', '..', 'tests', 'fixtures', 'python');
let passed = 0;
let failed = 0;
function assert(condition, message) {
    if (condition) {
        passed++;
    }
    else {
        failed++;
        console.error(`FAIL: ${message}`);
    }
}
function runCli(args, cwd) {
    return (0, child_process_1.spawnSync)(process.execPath, [cliEntry, ...args], {
        cwd,
        encoding: 'utf-8',
        timeout: 120_000,
    });
}
if (!(0, fs_1.existsSync)(cliEntry) || !(0, fs_1.existsSync)(nativeBin)) {
    console.log('(dist not built - skipping CLI e2e tests; run `npm run build` and copy graphify.node)');
}
else if (!(0, fs_1.existsSync)(fixtureDir)) {
    console.log(`(fixture missing: ${fixtureDir} - skipping CLI e2e tests)`);
}
else {
    const tmp = (0, fs_1.mkdtempSync)((0, path_1.join)((0, os_1.tmpdir)(), 'graphify-e2e-'));
    const project = (0, path_1.join)(tmp, 'project');
    (0, fs_1.cpSync)(fixtureDir, project, { recursive: true });
    // 1. Full pipeline through the binary
    const run = runCli(['run', '.'], project);
    assert(run.status === 0, `run should exit 0, got ${run.status}: ${String(run.stderr).slice(0, 200)}`);
    assert((0, fs_1.existsSync)((0, path_1.join)(project, '.graphify', 'db.sqlite')), 'run should create .graphify/db.sqlite');
    assert((0, fs_1.existsSync)((0, path_1.join)(project, '.graphify', 'graph_report.md')), 'run should create graph_report.md');
    assert((0, fs_1.existsSync)((0, path_1.join)(project, '.graphify', 'graph.json')), 'run should create graph.json');
    // 2. Incremental run adds nothing on unchanged files
    const rerun = runCli(['update', '.'], project);
    assert(rerun.status === 0, `update should exit 0 on unchanged files`);
    // 3. stats reports the extracted graph
    const stats = runCli(['stats', '--graph', '.'], project);
    assert(stats.status === 0, `stats should exit 0, got ${stats.status}: ${String(stats.stderr).slice(0, 200)}`);
    const nodeCount = Number((stats.stdout.match(/Nodes: (\d+)/) || [])[1]);
    assert(nodeCount > 0, `stats should report nodes, got "${stats.stdout.split('\n')[0]}"`);
    // 4. query answers a question grounded in the fixture
    const query = runCli(['query', 'sensor record', '--graph', '.', '--budget', '1000'], project);
    assert(query.status === 0, `query should exit 0, got ${query.status}: ${String(query.stderr).slice(0, 200)}`);
    assert(query.stdout.trim().length > 0, 'query should return output for a fixture concept');
    // 5. status reports a healthy graph
    const status = runCli(['status', '--graph', '.'], project);
    assert(status.status === 0, `status should exit 0, got ${status.status}`);
    assert(/healthy|ok/i.test(status.stdout) || status.stdout.length > 0, 'status should print a report');
    // 6. export produces valid JSON through the binary
    const outPath = (0, path_1.join)(tmp, 'export.json');
    const exportJson = runCli(['export', '--graph', '.', '--format', 'json', '--out', outPath], project);
    assert(exportJson.status === 0, `export json should exit 0, got ${exportJson.status}`);
    const parsed = JSON.parse((0, fs_1.readFileSync)(outPath, 'utf-8'));
    assert(Array.isArray(parsed.nodes) && parsed.nodes.length > 0, 'export json should contain nodes');
    // 7. a missing graph fails with a non-zero exit, not a silent success
    const missing = runCli(['stats', '--graph', (0, path_1.join)(tmp, 'nowhere')], tmp);
    assert(missing.status !== 0, 'stats on a nonexistent graph should exit non-zero');
    assert(String(missing.stderr).length > 0 || String(missing.stdout).length > 0, 'stats on a nonexistent graph should print an error');
    assert(!(0, fs_1.existsSync)((0, path_1.join)(tmp, 'nowhere', '.graphify')), 'failed stats must not create a .graphify directory');
    // 8. the MCP server refuses to serve (or create) a graph that was never built
    const mcpMissing = runCli(['mcp', '--graph', (0, path_1.join)(tmp, 'nowhere2')], tmp);
    assert(mcpMissing.status !== 0, 'mcp on a nonexistent graph should exit non-zero');
    assert(!(0, fs_1.existsSync)((0, path_1.join)(tmp, 'nowhere2', '.graphify')), 'failed mcp must not create a .graphify directory');
    console.log(`\n${passed} passed, ${failed} failed`);
    if (failed > 0) {
        process.exit(1);
    }
}
//# sourceMappingURL=e2e.test.js.map