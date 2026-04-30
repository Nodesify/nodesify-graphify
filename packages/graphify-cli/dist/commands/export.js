"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.exportCommand = exportCommand;
// @ts-ignore
const graphify_node_1 = require("../../graphify.node");
async function exportCommand(opts) {
    (0, graphify_node_1.exportJsonCmd)(opts.graph, opts.out);
    console.log(`Exported to: ${opts.out}`);
}
//# sourceMappingURL=export.js.map