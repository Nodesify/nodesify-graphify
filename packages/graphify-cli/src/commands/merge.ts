import * as path from 'path';
// @ts-ignore
import { mergeGraphs } from '../../graphify.node';

export async function mergeCommand(pathA: string, pathB: string, outPath: string) {
  console.log(`Merging graphs: ${pathA} + ${pathB} -> ${outPath}`);
  const result = mergeGraphs(pathA, pathB, outPath);
  console.log(`Nodes: ${result.nodesAdded}, Edges: ${result.edgesAdded}, Communities: ${result.communities}`);
  console.log(`Merged graph written to: ${path.join(outPath, '.graphify')}`);
}
