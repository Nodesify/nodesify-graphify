import * as pathMod from 'path';
import { runPipeline } from '../native';

export async function runCommand(path: string) {
  try {
    console.log(`Running graphify pipeline on: ${path}`);
    const result = runPipeline(path);
    console.log(`Nodes added: ${result.nodesAdded}`);
    console.log(`Edges added: ${result.edgesAdded}`);
    console.log(`Communities: ${result.communities}`);
    console.log(`Report written to: ${pathMod.join(path, '.graphify', 'graph_report.md')}`);
  } catch (e: any) {
    console.error(`Error: ${e.message || e}`);
    process.exitCode = 1;
  }
}
