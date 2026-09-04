import * as path from 'path';
import { exportWiki } from '../native';

export async function wikiCommand(opts: {
  graph: string;
  out: string;
  maxNodes: string;
}) {
  try {
    const maxNodes = parseInt(opts.maxNodes, 10) || 25;
    const count = exportWiki(opts.graph, opts.out, maxNodes);
    const resolved = path.isAbsolute(opts.out)
      ? opts.out
      : path.join(opts.graph, opts.out);
    console.log(`Wiki exported: ${count} articles -> ${resolved}`);
    console.log(`Start at: ${path.join(resolved, 'index.md')}`);
  } catch (e: any) {
    console.error(`Error: ${e.message || e}`);
    process.exitCode = 1;
  }
}
