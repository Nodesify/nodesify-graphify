// @ts-ignore
import { graphStats } from '../../graphify.node';

export async function statsCommand(opts: { graph: string }) {
  const stats = graphStats(opts.graph);
  console.log(`Nodes: ${stats.nodeCount}`);
  console.log(`Edges: ${stats.edgeCount}`);
  console.log(`Communities: ${stats.communityCount}`);
  console.log(`Files tracked: ${stats.fileCount}`);
}
