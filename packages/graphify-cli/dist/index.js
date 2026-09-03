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
const map_1 = require("./commands/map");
const affected_1 = require("./commands/affected");
const mcp_1 = require("./commands/mcp");
const tree_1 = require("./commands/tree");
const prs_1 = require("./commands/prs");
const add_1 = require("./commands/add");
const update_1 = require("./commands/update");
const watch_1 = require("./commands/watch");
const cluster_1 = require("./commands/cluster");
const merge_1 = require("./commands/merge");
const diff_1 = require("./commands/diff");
const history_1 = require("./commands/history");
const status_1 = require("./commands/status");
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
    .option('--no-dedup', 'Skip near-duplicate node merging')
    .option('--backend <name>', 'Semantic LLM backend: claude, openai (any OpenAI-compatible), or gemini')
    .option('--model <name>', 'Semantic LLM model name (backend-specific)')
    .action(run_1.runCommand);
program
    .command('update')
    .description('Run incremental AST-only rebuild')
    .argument('<path>', 'Directory to update')
    .option('--no-dedup', 'Skip near-duplicate node merging')
    .option('--backend <name>', 'Semantic LLM backend: claude, openai (any OpenAI-compatible), or gemini')
    .option('--model <name>', 'Semantic LLM model name (backend-specific)')
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
    .option('--depth <n>', 'Traversal depth', '2')
    .option('--budget <n>', 'Token budget for output', '2000')
    .option('--directed', 'Follow edges only in their stored direction (caller -> callee)')
    .option('--detail <level>', 'Fidelity tier: "high" keeps only EXTRACTED/DECLARED facts')
    .option('--cursor <n>', 'Continuation token from a previous truncated query', '0')
    .action(query_1.queryCommand);
program
    .command('path')
    .description('Find shortest path between two nodes')
    .argument('<source>', 'Source node label')
    .argument('<target>', 'Target node label')
    .option('--graph <path>', 'Path to project root', '.')
    .option('--directed', 'Follow edges only in their stored direction (caller -> callee)')
    .option('--detail <level>', 'Fidelity tier: "high" keeps only EXTRACTED/DECLARED facts')
    .action(path_1.pathCommand);
program
    .command('map')
    .description('Repo map: PageRank-ranked files with top symbols, within a token budget')
    .option('--graph <path>', 'Path to project root', '.')
    .option('--budget <n>', 'Token budget for output', '2000')
    .option('--detail <level>', 'Fidelity tier: "high" keeps only EXTRACTED/DECLARED facts')
    .action(map_1.mapCommand);
program
    .command('affected')
    .description('Show the blast radius of a node — everything impacted by changing it')
    .argument('<node>', 'Node ID, label, or source file path')
    .option('--graph <path>', 'Path to project root', '.')
    .option('--depth <n>', 'Maximum hops to traverse', '2')
    .option('--relation <type>', 'Only follow one relation (e.g. calls, imports, uses)')
    .action(affected_1.affectedCommand);
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
program
    .command('mcp')
    .description('Run an MCP stdio server exposing the graph to AI agents (Claude, etc.)')
    .option('--graph <path>', 'Path to project root', '.')
    .action(mcp_1.mcpCommand);
program
    .command('tree')
    .description('Export a collapsible filesystem tree of all graph symbols (self-contained HTML)')
    .option('--graph <path>', 'Path to project root', '.')
    .option('--out <file>', 'Output HTML file', 'tree.html')
    .option('--max-children <n>', 'Max symbols shown per directory', '40')
    .action(tree_1.treeCommand);
program
    .command('prs')
    .description('Map open pull requests onto the knowledge graph (impact + merge-order risk)')
    .argument('[count]', 'Number of PRs to analyze', '20')
    .option('--graph <path>', 'Path to project root', '.')
    .option('--conflicts', 'Flag PRs sharing communities (merge-order risk)')
    .action(prs_1.prsCommand);
program
    .command('add')
    .description('Fetch a URL (arXiv paper, tweet, webpage, image, PDF) into ./raw and update the graph')
    .argument('<url>', 'URL to fetch')
    .option('--graph <path>', 'Path to project root', '.')
    .option('--author <name>', 'Author recorded in the saved metadata')
    .option('--contributor <name>', 'Contributor recorded in the saved metadata')
    .action(add_1.addCommand);
program
    .command('status')
    .description('Check graph health and staleness')
    .option('--graph <path>', 'Path to project root', '.')
    .action(status_1.statusCommand);
(0, install_1.registerInstallCommand)(program);
(0, hook_1.registerHookCommand)(program);
program.parse();
//# sourceMappingURL=index.js.map