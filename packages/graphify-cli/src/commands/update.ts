import * as pathMod from 'path';
import { updatePipeline } from '../native';

export async function updateCommand(path: string) {
  try {
    console.log(`Running incremental rebuild on: ${path}`);
    const result = updatePipeline(path);
    console.log(`Nodes: ${result.nodesAdded}, Edges: ${result.edgesAdded}, Communities: ${result.communities}`);
    console.log(`Report updated at: ${pathMod.join(path, '.graphify', 'graph_report.md')}`);
  } catch (e: any) {
    console.error(`Error: ${e.message || e}`);
    process.exitCode = 1;
  }
}
