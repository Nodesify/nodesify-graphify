#!/usr/bin/env node
"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.program = void 0;
const commander_1 = require("commander");
const run_1 = require("./commands/run");
const stats_1 = require("./commands/stats");
const explain_1 = require("./commands/explain");
const export_1 = require("./commands/export");
const query_1 = require("./commands/query");
const path_1 = require("./commands/path");
const update_1 = require("./commands/update");
const watch_1 = require("./commands/watch");
const cluster_1 = require("./commands/cluster");
const merge_1 = require("./commands/merge");
const diff_1 = require("./commands/diff");
const history_1 = require("./commands/history");
const install_1 = require("./commands/install");
const hook_1 = require("./commands/hook");
const program = new commander_1.Command();
exports.program = program;
program
    .name('nodesify-graphify')
    .description('Turn any folder into a queryable knowledge graph')
    .version(require('../package.json').version);
program
    .command('run')
    .description('Run the full pipeline on a directory')
    .argument('<path>', 'Directory to analyze')
    .action(run_1.runCommand);
program
    .command('update')
    .description('Run incremental AST-only rebuild')
    .argument('<path>', 'Directory to update')
    .action(update_1.updateCommand);
program
    .command('watch')
    .description('Watch for file changes and auto-rebuild')
    .argument('<path>', 'Directory to watch')
    .option('--debounce <ms>', 'Debounce interval in milliseconds', '3000')
    .action(watch_1.watchCommand);
program
    .command('explain')
    .description('Explain a node and its connections')
    .argument('<node>', 'Node ID or label')
    .option('--graph <path>', 'Path to project root', '.')
    .action(explain_1.explainCommand);
program
    .command('query')
    .description('BFS/DFS graph traversal for a question')
    .argument('<question>', 'Search terms')
    .option('--graph <path>', 'Path to project root', '.')
    .option('--dfs', 'Use depth-first search instead of breadth-first')
    .option('--budget <n>', 'Token budget for output', '2000')
    .action(query_1.queryCommand);
program
    .command('path')
    .description('Find shortest path between two nodes')
    .argument('<source>', 'Source node label')
    .argument('<target>', 'Target node label')
    .option('--graph <path>', 'Path to project root', '.')
    .action(path_1.pathCommand);
program
    .command('stats')
    .description('Show graph statistics')
    .option('--graph <path>', 'Path to project root', '.')
    .action(stats_1.statsCommand);
program
    .command('export')
    .description('Export graph to JSON, HTML, or GraphML')
    .option('--graph <path>', 'Path to project root', '.')
    .option('--out <file>', 'Output file', 'graph.json')
    .option('--format <type>', 'Export format: json, html, graphml', 'json')
    .action(export_1.exportCommand);
program
    .command('cluster-only')
    .description('Run cluster + analyze + report only (no extract/build)')
    .argument('<path>', 'Directory with existing graph')
    .action(cluster_1.clusterCommand);
program
    .command('merge')
    .description('Merge two graphs into a new output graph')
    .argument('<pathA>', 'First project root')
    .argument('<pathB>', 'Second project root')
    .argument('<outPath>', 'Output project root')
    .action(merge_1.mergeCommand);
program
    .command('diff')
    .description('Compare two graphs and show differences')
    .argument('<pathA>', 'First project root')
    .argument('<pathB>', 'Second project root')
    .action(diff_1.diffCommand);
program
    .command('history')
    .description('Show recent query history')
    .option('--limit <n>', 'Number of entries to show', '20')
    .option('--graph <path>', 'Path to project root', '.')
    .action(history_1.historyCommand);
(0, install_1.registerInstallCommand)(program);
(0, hook_1.registerHookCommand)(program);
program.parse();
//# sourceMappingURL=index.js.map