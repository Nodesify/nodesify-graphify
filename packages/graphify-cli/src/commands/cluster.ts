import * as pathMod from 'path';
// @ts-ignore
import { clusterOnly } from '../../graphify.node';

export async function clusterCommand(path: string) {
  console.log(`Running cluster + analyze on: ${path}`);
  const result = clusterOnly(path);
  console.log(`Communities: ${result.communities}`);
  console.log(`Report updated at: ${pathMod.join(path, '.graphify', 'graph_report.md')}`);
}
