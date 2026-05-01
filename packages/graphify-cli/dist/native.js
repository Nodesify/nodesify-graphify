"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.graphHistory = exports.diffGraphs = exports.mergeGraphs = exports.clusterOnly = exports.findPath = exports.queryGraph = exports.exportGraphmlCmd = exports.exportHtmlCmd = exports.exportJsonCmd = exports.explainNode = exports.graphStats = exports.updatePipeline = exports.runPipeline = void 0;
const path_1 = require("path");
const fs_1 = require("fs");
const PLATFORM_SUFFIX = {
    'win32-x64': 'win32-x64-msvc',
    'darwin-x64': 'darwin-x64',
    'darwin-arm64': 'darwin-arm64',
    'linux-x64': 'linux-x64-gnu',
    'linux-arm64': 'linux-arm64-gnu',
};
function isMusl() {
    try {
        const { execSync } = require('child_process');
        const out = execSync('ldd --version 2>&1 || true', { encoding: 'utf-8' });
        return out.includes('musl');
    }
    catch {
        return false;
    }
}
function getPlatformSuffix() {
    if (process.platform === 'linux' && isMusl()) {
        return `linux-${process.arch}-musl`;
    }
    return PLATFORM_SUFFIX[`${process.platform}-${process.arch}`] || `${process.platform}-${process.arch}`;
}
function loadNativeBinding() {
    for (const candidate of [
        (0, path_1.join)(__dirname, '..', 'graphify.node'),
        (0, path_1.join)(__dirname, 'graphify.node'),
    ]) {
        if ((0, fs_1.existsSync)(candidate))
            return require(candidate);
    }
    const suffix = getPlatformSuffix();
    const pkgName = `@nodesify/graphify-${suffix}`;
    try {
        return require(pkgName);
    }
    catch { }
    throw new Error(`@nodesify/graphify: failed to load native module for ${process.platform}-${process.arch}.\n` +
        `Tried: local graphify.node, ${pkgName}\n` +
        `Ensure the correct platform package is installed.`);
}
const binding = loadNativeBinding();
exports.runPipeline = binding.runPipeline;
exports.updatePipeline = binding.updatePipeline;
exports.graphStats = binding.graphStats;
exports.explainNode = binding.explainNode;
exports.exportJsonCmd = binding.exportJsonCmd;
exports.exportHtmlCmd = binding.exportHtmlCmd;
exports.exportGraphmlCmd = binding.exportGraphmlCmd;
exports.queryGraph = binding.queryGraph;
exports.findPath = binding.findPath;
exports.clusterOnly = binding.clusterOnly;
exports.mergeGraphs = binding.mergeGraphs;
exports.diffGraphs = binding.diffGraphs;
exports.graphHistory = binding.graphHistory;
//# sourceMappingURL=native.js.map