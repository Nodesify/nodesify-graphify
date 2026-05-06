#!/usr/bin/env node

import { Command } from 'commander';
import { runCommand } from './commands/run';
import { statsCommand } from './commands/stats';
import { explainCommand } from './commands/explain';
import { exportCommand } from './commands/export';
import { queryCommand } from './commands/query';
import { pathCommand } from './commands/path';
import { updateCommand } from './commands/update';
import { watchCommand } from './commands/watch';
import { clusterCommand } from './commands/cluster';
import { mergeCommand } from './commands/merge';
import { diffCommand } from './commands/diff';
import { historyCommand } from './commands/history';
import { registerInstallCommand } from './commands/install';
import { registerHookCommand } from './commands/hook';

const program = new Command();

program
  .name('nodesify-graphify')
  .description('Turn any folder into a queryable knowledge graph')
  .version(require('../package.json').version);

program
  .command('run')
  .description('Run the full pipeline on a directory')
  .argument('<path>', 'Directory to analyze')
  .action(runCommand);

program
  .command('update')
  .description('Run incremental AST-only rebuild')
  .argument('<path>', 'Directory to update')
  .action(updateCommand);

program
  .command('watch')
  .description('Watch for file changes and auto-rebuild')
  .argument('<path>', 'Directory to watch')
  .option('--debounce <ms>', 'Debounce interval in milliseconds', '3000')
  .action(watchCommand);

program
  .command('explain')
  .description('Explain a node and its connections')
  .argument('<node>', 'Node ID or label')
  .option('--graph <path>', 'Path to project root', '.')
  .action(explainCommand);

program
  .command('query')
  .description('BFS/DFS graph traversal for a question')
  .argument('<question>', 'Search terms')
  .option('--graph <path>', 'Path to project root', '.')
  .option('--dfs', 'Use depth-first search instead of breadth-first')
  .option('--depth <n>', 'Traversal depth', '2')
  .option('--budget <n>', 'Token budget for output', '2000')
  .action(queryCommand);

program
  .command('path')
  .description('Find shortest path between two nodes')
  .argument('<source>', 'Source node label')
  .argument('<target>', 'Target node label')
  .option('--graph <path>', 'Path to project root', '.')
  .action(pathCommand);

program
  .command('stats')
  .description('Show graph statistics')
  .option('--graph <path>', 'Path to project root', '.')
  .action(statsCommand);

program
  .command('export')
  .description('Export graph to JSON, HTML, or GraphML')
  .option('--graph <path>', 'Path to project root', '.')
  .option('--out <file>', 'Output file', 'graph.json')
  .option('--format <type>', 'Export format: json, html, graphml', 'json')
  .action(exportCommand);

program
  .command('cluster-only')
  .description('Run cluster + analyze + report only (no extract/build)')
  .argument('<path>', 'Directory with existing graph')
  .action(clusterCommand);

program
  .command('merge')
  .description('Merge two graphs into a new output graph')
  .argument('<pathA>', 'First project root')
  .argument('<pathB>', 'Second project root')
  .argument('<outPath>', 'Output project root')
  .action(mergeCommand);

program
  .command('diff')
  .description('Compare two graphs and show differences')
  .argument('<pathA>', 'First project root')
  .argument('<pathB>', 'Second project root')
  .action(diffCommand);

program
  .command('history')
  .description('Show recent query history')
  .option('--limit <n>', 'Number of entries to show', '20')
  .option('--graph <path>', 'Path to project root', '.')
  .action(historyCommand);

registerInstallCommand(program);
registerHookCommand(program);

program.parse();

export { program };
