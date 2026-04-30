"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.explainCommand = explainCommand;
// @ts-ignore
const graphify_node_1 = require("../../graphify.node");
async function explainCommand(node, opts) {
    const result = (0, graphify_node_1.getNode)(opts.graph, node);
    if (!result) {
        console.log(`Node "${node}" not found`);
        return;
    }
    console.log(`Node: ${result.label}`);
    console.log(`  File: ${result.sourceFile}:${result.sourceLine ?? '?'}`);
    console.log(`  Type: ${result.fileType}`);
    if (result.docstring)
        console.log(`  Docstring: ${result.docstring}`);
    if (result.community !== null && result.community !== undefined)
        console.log(`  Community: ${result.community}`);
}
//# sourceMappingURL=explain.js.map