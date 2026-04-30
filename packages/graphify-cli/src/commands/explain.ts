// @ts-ignore
import { getNode } from '../../graphify.node';

export async function explainCommand(node: string, opts: { graph: string }) {
  const result = getNode(opts.graph, node);
  if (!result) {
    console.log(`Node "${node}" not found`);
    return;
  }
  console.log(`Node: ${result.label}`);
  console.log(`  File: ${result.sourceFile}:${result.sourceLine ?? '?'}`);
  console.log(`  Type: ${result.fileType}`);
  if (result.docstring) console.log(`  Docstring: ${result.docstring}`);
  if (result.community !== null && result.community !== undefined) console.log(`  Community: ${result.community}`);
}
