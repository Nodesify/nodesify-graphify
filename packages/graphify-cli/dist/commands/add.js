"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.addCommand = addCommand;
const native_1 = require("../native");
async function addCommand(url, opts) {
    try {
        console.log(`Fetching ${url}...`);
        const result = (0, native_1.ingestUrl)(opts.graph, url, opts.author, opts.contributor);
        console.log(`Saved: ${result.savedPath}`);
        if (result.graphUpdated) {
            console.log('Graph updated with the new content.');
        }
    }
    catch (e) {
        console.error(`Error: ${e.message || e}`);
        process.exitCode = 1;
    }
}
//# sourceMappingURL=add.js.map