import { queryGraph } from '../native';

export async function queryCommand(question: string, opts: {
  graph: string;
  dfs: boolean;
  budget: string;
}) {
  try {
    const mode = opts.dfs ? 'dfs' : 'bfs';
    const budget = parseInt(opts.budget || '2000', 10);
    const result = queryGraph(opts.graph, question, mode, 2, budget);
    console.log(result.text);
  } catch (e: any) {
    console.error(`Error: ${e.message || e}`);
    process.exitCode = 1;
  }
}
