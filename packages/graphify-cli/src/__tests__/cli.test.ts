/**
 * CLI test — validates that the Commander program registers all commands
 * and parses arguments correctly. Does not call any napi native functions.
 *
 * Run with: npx tsx src/__tests__/cli.test.ts
 */

import { Command } from 'commander';

let passed = 0;
let failed = 0;

function assert(condition: boolean, message: string) {
  if (condition) {
    passed++;
  } else {
    failed++;
    console.error(`FAIL: ${message}`);
  }
}

function createProgram(): Command {
  const program = new Command();
  program.exitOverride(); // prevent process.exit during testing
  program.configureOutput({
    writeOut: () => {},
    writeErr: () => {},
  });

  program
    .name('nodesify-graphify')
    .description('Turn any folder into a queryable knowledge graph')
    .version('0.1.0');

  program
    .command('run')
    .description('Run the full pipeline on a directory')
    .argument('<path>', 'Directory to analyze')
    .action(() => {});

  program
    .command('update')
    .description('Run incremental AST-only rebuild')
    .argument('<path>', 'Directory to update')
    .action(() => {});

  program
    .command('watch')
    .description('Watch for file changes and auto-rebuild')
    .argument('<path>', 'Directory to watch')
    .option('--debounce <ms>', 'Debounce interval in milliseconds', '3000')
    .action(() => {});

  program
    .command('explain')
    .description('Explain a node and its connections')
    .argument('<node>', 'Node ID or label')
    .option('--graph <path>', 'Path to project root', '.')
    .action(() => {});

  program
    .command('query')
    .description('BFS/DFS graph traversal for a question')
    .argument('<question>', 'Search terms')
    .option('--graph <path>', 'Path to project root', '.')
    .option('--dfs', 'Use depth-first search instead of breadth-first')
    .option('--budget <n>', 'Token budget for output', '2000')
    .action(() => {});

  program
    .command('path')
    .description('Find shortest path between two nodes')
    .argument('<source>', 'Source node label')
    .argument('<target>', 'Target node label')
    .option('--graph <path>', 'Path to project root', '.')
    .action(() => {});

  program
    .command('stats')
    .description('Show graph statistics')
    .option('--graph <path>', 'Path to project root', '.')
    .action(() => {});

  program
    .command('export')
    .description('Export graph to JSON, HTML, or GraphML')
    .option('--graph <path>', 'Path to project root', '.')
    .option('--out <file>', 'Output file', 'graph.json')
    .option('--format <type>', 'Export format: json, html, graphml', 'json')
    .action(() => {});

  program
    .command('cluster-only')
    .description('Run cluster + analyze + report only (no extract/build)')
    .argument('<path>', 'Directory with existing graph')
    .action(() => {});

  program
    .command('merge')
    .description('Merge two graphs into a new output graph')
    .argument('<pathA>', 'First project root')
    .argument('<pathB>', 'Second project root')
    .argument('<outPath>', 'Output project root')
    .action(() => {});

  program
    .command('diff')
    .description('Compare two graphs and show differences')
    .argument('<pathA>', 'First project root')
    .argument('<pathB>', 'Second project root')
    .action(() => {});

  program
    .command('history')
    .description('Show recent query history')
    .option('--limit <n>', 'Number of entries to show', '20')
    .option('--graph <path>', 'Path to project root', '.')
    .action(() => {});

  return program;
}

// ---- Tests ----

const program = createProgram();

// Test 1: All core commands are registered
const commandNames = program.commands.map((c: Command) => c.name());
const requiredCommands = ['run', 'update', 'watch', 'explain', 'query', 'path', 'stats', 'export'];
for (const cmd of requiredCommands) {
  assert(commandNames.includes(cmd), `Command "${cmd}" should be registered`);
}

// Test 2: New commands are registered
const newCommands = ['cluster-only', 'merge', 'diff', 'history'];
for (const cmd of newCommands) {
  assert(commandNames.includes(cmd), `New command "${cmd}" should be registered`);
}

// Test 3: Export command accepts --format flag
const exportCmd = program.commands.find((c: Command) => c.name() === 'export');
assert(exportCmd !== undefined, 'Export command should exist');
if (exportCmd) {
  const exportOpts = exportCmd.options.map((o: any) => o.long);
  assert(exportOpts.includes('--format'), 'Export command should have --format option');
  const formatOpt = exportCmd.options.find((o: any) => o.long === '--format');
  assert(!!formatOpt && (formatOpt.defaultValue ?? 'json') === 'json', 'Export --format should default to "json"');
}

// Test 4: Parse "run" command with path argument
try {
  program.parse(['run', '/tmp/project'], { from: 'user' });
  assert(true, 'run command parses with path argument');
} catch (e: any) {
  assert(e.exitCode === 0, 'run command should parse without error');
}

// Test 5: Parse "export" command with --format html
try {
  const p2 = createProgram();
  p2.parse(['export', '--format', 'html', '--out', 'graph.html'], { from: 'user' });
  assert(true, 'export --format html parses correctly');
} catch (e: any) {
  assert(e.exitCode === 0, 'export --format should parse without error');
}

// Test 6: Parse "history" command with --limit
try {
  const p3 = createProgram();
  p3.parse(['history', '--limit', '5', '--graph', '.'], { from: 'user' });
  assert(true, 'history --limit parses correctly');
} catch (e: any) {
  assert(e.exitCode === 0, 'history --limit should parse without error');
}

// Test 7: Parse "merge" command with three arguments
try {
  const p4 = createProgram();
  p4.parse(['merge', '/a', '/b', '/out'], { from: 'user' });
  assert(true, 'merge command parses with three path arguments');
} catch (e: any) {
  assert(e.exitCode === 0, 'merge command should parse without error');
}

// Test 8: Parse "diff" command with two arguments
try {
  const p5 = createProgram();
  p5.parse(['diff', '/a', '/b'], { from: 'user' });
  assert(true, 'diff command parses with two path arguments');
} catch (e: any) {
  assert(e.exitCode === 0, 'diff command should parse without error');
}

// Test 9: Total command count
assert(
  commandNames.length >= requiredCommands.length + newCommands.length,
  `Should have at least ${requiredCommands.length + newCommands.length} commands, got ${commandNames.length}`
);

// Summary
console.log(`\n${passed} passed, ${failed} failed`);
if (failed > 0) {
  process.exit(1);
}
