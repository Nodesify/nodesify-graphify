"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.runCommand = runCommand;
// @ts-ignore
const graphify_node_1 = require("../../graphify.node");
async function runCommand(path) {
    console.log(`Running graphify pipeline on: ${path}`);
    const result = (0, graphify_node_1.runPipeline)(path);
    console.log(`Nodes added: ${result.nodesAdded}`);
    console.log(`Edges added: ${result.edgesAdded}`);
    console.log(`Communities: ${result.communities}`);
    console.log(`Report written to: ${path}/.graphify/graph_report.md`);
}
//# sourceMappingURL=run.js.map