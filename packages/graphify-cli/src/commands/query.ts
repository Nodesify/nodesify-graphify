import { queryGraph } from '../native';

export async function queryCommand(question: string, opts: {
  graph: string;
  dfs: boolean;
  depth: string;
  budget: string;
  directed?: boolean;
}) {
  try {
    const mode = opts.dfs ? 'dfs' : 'bfs';
    const depth = parseInt(opts.depth || '2', 10);
    const budget = parseInt(opts.budget || '2000', 10);
    const result = queryGraph(opts.graph, question, mode, depth, budget, opts.directed ?? false);
    console.log(result.text);
  } catch (e: any) {
    console.error(`Error: ${e.message || e}`);
    process.exitCode = 1;
  }
}
