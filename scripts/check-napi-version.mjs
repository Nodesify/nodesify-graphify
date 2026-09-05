// CI guard against @napi-rs/cli version drift (see #48, #49): package-lock.json
// must resolve the same napi v2 the release workflow pins. Checks v2 only —
// widen if the workflow ever moves to v3.
import { createReadStream, readFileSync } from 'node:fs';
import { createInterface } from 'node:readline';

const { devDependencies } = JSON.parse(readFileSync('packages/graphify-cli/package.json', 'utf8'));
const declared = devDependencies['@napi-rs/cli'];

// Stream the lockfile (megabytes) instead of parsing it.
let resolved;
let inEntry = false;
for await (const line of createInterface({ input: createReadStream('package-lock.json') })) {
  if (line.includes('node_modules/@napi-rs/cli"')) inEntry = true;
  else if (inEntry && line.includes('"version":')) {
    resolved = line.match(/"version":\s*"([^"]+)"/)?.[1];
    break;
  }
}

const isV2 = (v) => /^\^?2\./.test(v);
if (!isV2(declared ?? '') || !isV2(resolved ?? '')) {
  console.error(
    `@napi-rs/cli drift: declared "${declared ?? '<missing>'}" in packages/graphify-cli/package.json, ` +
      `resolved "${resolved ?? '<not found>'}" in package-lock.json; the release workflow requires napi v2 (see #48, #49)`,
  );
  process.exit(1);
}
