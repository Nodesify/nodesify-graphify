import { join } from 'path';
import { existsSync } from 'fs';

const PLATFORM_SUFFIX: Record<string, string> = {
  'win32-x64': 'win32-x64-msvc',
  'darwin-x64': 'darwin-x64',
  'darwin-arm64': 'darwin-arm64',
  'linux-x64': 'linux-x64-gnu',
  'linux-arm64': 'linux-arm64-gnu',
};

function isMusl(): boolean {
  try {
    const { execSync } = require('child_process') as typeof import('child_process');
    const out = execSync('ldd --version 2>&1 || true', { encoding: 'utf-8' });
    return out.includes('musl');
  } catch {
    return false;
  }
}

function getPlatformSuffix(): string {
  if (process.platform === 'linux' && isMusl()) {
    return `linux-${process.arch}-musl`;
  }
  return PLATFORM_SUFFIX[`${process.platform}-${process.arch}`] || `${process.platform}-${process.arch}`;
}

function loadNativeBinding(): any {
  for (const candidate of [
    join(__dirname, '..', 'graphify.node'),
    join(__dirname, 'graphify.node'),
  ]) {
    if (existsSync(candidate)) return require(candidate);
  }

  const suffix = getPlatformSuffix();
  const pkgName = `@nodesify/graphify-${suffix}`;
  try {
    return require(pkgName);
  } catch {}

  throw new Error(
    `@nodesify/graphify: failed to load native module for ${process.platform}-${process.arch}.\n` +
    `Tried: local graphify.node, ${pkgName}\n` +
    `Ensure the correct platform package is installed.`,
  );
}

const binding = loadNativeBinding();

export const runPipeline = binding.runPipeline;
export const updatePipeline = binding.updatePipeline;
export const graphStats = binding.graphStats;
export const explainNode = binding.explainNode;
export const exportJsonCmd = binding.exportJsonCmd;
export const exportHtmlCmd = binding.exportHtmlCmd;
export const exportGraphmlCmd = binding.exportGraphmlCmd;
export const queryGraph = binding.queryGraph;
export const findPath = binding.findPath;
export const clusterOnly = binding.clusterOnly;
export const mergeGraphs = binding.mergeGraphs;
export const diffGraphs = binding.diffGraphs;
export const graphHistory = binding.graphHistory;
