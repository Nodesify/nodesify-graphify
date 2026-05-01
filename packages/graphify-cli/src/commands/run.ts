import * as pathMod from 'path';
// @ts-ignore
import { runPipeline } from '../../graphify.node';

export async function runCommand(path: string) {
  console.log(`Running graphify pipeline on: ${path}`);
  const result = runPipeline(path);
  console.log(`Nodes added: ${result.nodesAdded}`);
  console.log(`Edges added: ${result.edgesAdded}`);
  console.log(`Communities: ${result.communities}`);
  console.log(`Report written to: ${pathMod.join(path, '.graphify', 'graph_report.md')}`);
}
