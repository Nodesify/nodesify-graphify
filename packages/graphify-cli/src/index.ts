#!/usr/bin/env node

import { Command } from 'commander';
import { runCommand } from './commands/run';
import { statsCommand } from './commands/stats';
import { explainCommand } from './commands/explain';
import { exportCommand } from './commands/export';

const program = new Command();

program
  .name('graphify')
  .description('Turn any folder into a queryable knowledge graph')
  .version('0.1.0');

program
  .command('run')
  .description('Run the full pipeline on a directory')
  .argument('<path>', 'Directory to analyze')
  .action(runCommand);

program
  .command('explain')
  .description('Explain a node in plain language')
  .argument('<node>', 'Node ID or label')
  .option('--graph <path>', 'Path to project root', '.')
  .action(explainCommand);

program
  .command('stats')
  .description('Show graph statistics')
  .option('--graph <path>', 'Path to project root', '.')
  .action(statsCommand);

program
  .command('export')
  .description('Export graph to JSON')
  .option('--graph <path>', 'Path to project root', '.')
  .option('--out <file>', 'Output file', 'graph.json')
  .action(exportCommand);

program.parse();
