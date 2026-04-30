"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.statsCommand = statsCommand;
// @ts-ignore
const graphify_node_1 = require("../../graphify.node");
async function statsCommand(opts) {
    const stats = (0, graphify_node_1.graphStats)(opts.graph);
    console.log(`Nodes: ${stats.nodeCount}`);
    console.log(`Edges: ${stats.edgeCount}`);
    console.log(`Communities: ${stats.communityCount}`);
    console.log(`Files tracked: ${stats.fileCount}`);
}
//# sourceMappingURL=stats.js.map