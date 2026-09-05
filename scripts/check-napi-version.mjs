#!/usr/bin/env node
// Pre-publish guard for @napi-rs/cli major drift (#48, #49): the release
// workflow drives the v2 CLI (NAPI_RS_CLI_VERSION, `npx napi create-npm-dir`),
// but the caret range in packages/graphify-cli can silently resolve to v3.
// Guard checks v2 only — widen if the workflow ever moves to v3.
import { createReadStream } from 'node:fs';
import { readFile } from 'node:fs/promises';
import readline from 'node:readline';

const fail = (msg) => {
  console.error(msg);
  process.exit(1);
};
const major = (v) => /^[\^~]?(\d+)/.exec(v)?.[1];

const pkg = JSON.parse(await readFile('packages/graphify-cli/package.json', 'utf8'));
const declared = pkg.devDependencies['@napi-rs/cli'];
if (!declared) fail('@napi-rs/cli missing from packages/graphify-cli devDependencies');

// Stream the lockfile line-by-line (it is megabytes) and grab the "version"
// field of the node_modules/@napi-rs/cli entry (nested key under the package).
let resolved;
let inEntry = false;
const lines = readline.createInterface({
  input: createReadStream('package-lock.json'),
  crlfDelay: Infinity,
});
for await (const line of lines) {
  if (inEntry) {
    resolved = /"version": "([^"]+)"/.exec(line)?.[1];
    // lockfile v3 lists "version" as the first field of every entry
    if (resolved) break;
    if (line.trimEnd().endsWith('": {')) break; // walked past our entry
  } else if (line.includes('node_modules/@napi-rs/cli": {')) {
    inEntry = true;
  }
}
if (!resolved) fail('@napi-rs/cli entry not found in package-lock.json');

if (major(declared) !== '2' || major(resolved) !== '2') {
  fail(
    `@napi-rs/cli drift: declared "${declared}" vs resolved "${resolved}" ` +
      `— the release workflow requires the v2 CLI (see #48, #49).`,
  );
}
