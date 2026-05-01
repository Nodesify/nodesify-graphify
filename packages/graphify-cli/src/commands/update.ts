import * as pathMod from 'path';
// @ts-ignore
import { updatePipeline } from '../../graphify.node';

export async function updateCommand(path: string) {
  console.log(`Running incremental rebuild on: ${path}`);
  const result = updatePipeline(path);
  console.log(`Nodes: ${result.nodesAdded}, Edges: ${result.edgesAdded}, Communities: ${result.communities}`);
  console.log(`Report updated at: ${pathMod.join(path, '.graphify', 'graph_report.md')}`);
}
