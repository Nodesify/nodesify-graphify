#!/usr/bin/env node
"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
const commander_1 = require("commander");
const run_1 = require("./commands/run");
const stats_1 = require("./commands/stats");
const explain_1 = require("./commands/explain");
const export_1 = require("./commands/export");
const program = new commander_1.Command();
program
    .name('graphify')
    .description('Turn any folder into a queryable knowledge graph')
    .version('0.1.0');
program
    .command('run')
    .description('Run the full pipeline on a directory')
    .argument('<path>', 'Directory to analyze')
    .action(run_1.runCommand);
program
    .command('explain')
    .description('Explain a node in plain language')
    .argument('<node>', 'Node ID or label')
    .option('--graph <path>', 'Path to project root', '.')
    .action(explain_1.explainCommand);
program
    .command('stats')
    .description('Show graph statistics')
    .option('--graph <path>', 'Path to project root', '.')
    .action(stats_1.statsCommand);
program
    .command('export')
    .description('Export graph to JSON')
    .option('--graph <path>', 'Path to project root', '.')
    .option('--out <file>', 'Output file', 'graph.json')
    .action(export_1.exportCommand);
program.parse();
//# sourceMappingURL=index.js.map