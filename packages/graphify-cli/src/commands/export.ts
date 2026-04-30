// @ts-ignore
import { exportJsonCmd } from '../../graphify.node';

export async function exportCommand(opts: { graph: string; out: string }) {
  exportJsonCmd(opts.graph, opts.out);
  console.log(`Exported to: ${opts.out}`);
}
