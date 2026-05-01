import { findPath } from '../native';

export async function pathCommand(source: string, target: string, opts: {
  graph: string;
}) {
  try {
    const result = findPath(opts.graph, source, target);
    if (!result.found) {
      console.log(result.text);
      return;
    }
    console.log(result.text);
  } catch (e: any) {
    console.error(`Error: ${e.message || e}`);
    process.exitCode = 1;
  }
}
