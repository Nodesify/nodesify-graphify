"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.queryCommand = queryCommand;
const native_1 = require("../native");
async function queryCommand(question, opts) {
    try {
        const mode = opts.dfs ? 'dfs' : 'bfs';
        const depth = parseInt(opts.depth || '2', 10);
        const budget = parseInt(opts.budget || '2000', 10);
        const result = (0, native_1.queryGraph)(opts.graph, question, mode, depth, budget);
        console.log(result.text);
    }
    catch (e) {
        console.error(`Error: ${e.message || e}`);
        process.exitCode = 1;
    }
}
//# sourceMappingURL=query.js.map