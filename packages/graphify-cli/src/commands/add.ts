import { ingestUrl } from '../native';

export async function addCommand(url: string, opts: {
  graph: string;
  author?: string;
  contributor?: string;
}) {
  try {
    console.log(`Fetching ${url}...`);
    const result = ingestUrl(opts.graph, url, opts.author, opts.contributor);
    console.log(`Saved: ${result.savedPath}`);
    if (result.graphUpdated) {
      console.log('Graph updated with the new content.');
    }
  } catch (e: any) {
    console.error(`Error: ${e.message || e}`);
    process.exitCode = 1;
  }
}
