import * as fs from 'fs';
import * as path from 'path';
// @ts-ignore
import { runPipeline } from '../../graphify.node';

const CODE_EXTENSIONS = new Set([
  '.py', '.js', '.jsx', '.mjs', '.ts', '.tsx',
  '.rs', '.go', '.java', '.c', '.h', '.cpp', '.cc', '.cxx', '.hpp',
]);

const SKIP_DIRS = new Set(['.graphify', 'node_modules', 'target', '.git', 'dist', '__pycache__']);

export async function watchCommand(watchPath: string, opts: { debounce: string }) {
  const debounceMs = parseInt(opts.debounce || '3000', 10);
  const resolved = path.resolve(watchPath);

  let changedFiles = new Set<string>();
  let timer: ReturnType<typeof setTimeout> | null = null;

  const watcher = fs.watch(resolved, { recursive: true }, (_event, filename) => {
    if (!filename) return;
    const filePath = filename.replace(/\\/g, '/');
    const parts = filePath.split('/');

    if (parts.some((p: string) => SKIP_DIRS.has(p))) return;

    const ext = path.extname(filePath).toLowerCase();
    if (!CODE_EXTENSIONS.has(ext)) return;

    changedFiles.add(filePath);

    if (timer) clearTimeout(timer);
    timer = setTimeout(() => {
      const batch = [...changedFiles];
      changedFiles.clear();
      console.log(`\n[nodesify-graphify] ${batch.length} file(s) changed, rebuilding...`);
      try {
        const result = runPipeline(resolved);
        console.log(`[nodesify-graphify] Rebuilt: ${result.nodesAdded} nodes, ${result.edgesAdded} edges, ${result.communities} communities`);
      } catch (e: any) {
        console.error(`[nodesify-graphify] Rebuild failed:`, e.message || e);
      }
    }, debounceMs);
  });

  console.log(`[nodesify-graphify] Watching ${resolved} (debounce: ${debounceMs}ms)`);
  console.log('[nodesify-graphify] Press Ctrl+C to stop');

  if (process.platform === 'win32') {
    const readline = require('readline');
    const rl = readline.createInterface({ input: process.stdin });
    rl.on('SIGINT', () => {
      console.log('\n[nodesify-graphify] Stopped.');
      watcher.close();
      rl.close();
      process.exit(0);
    });
  } else {
    process.on('SIGINT', () => {
      console.log('\n[nodesify-graphify] Stopped.');
      watcher.close();
      process.exit(0);
    });
    process.on('SIGTERM', () => {
      console.log('\n[nodesify-graphify] Stopped.');
      watcher.close();
      process.exit(0);
    });
  }
}
