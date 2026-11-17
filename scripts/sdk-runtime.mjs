// Compile the TypeScript SDK runtime (src/calculate/sdk_ts_runtime.ts) into the
// ES module and declarations that `runa sdk runtime` writes as @futuruna/runtime
// (src/calculate/sdk_ts_runtime.js and .d.ts, which runa embeds).
//
//   FUTURUNA_SDK_TS_DEPS=<dir> node scripts/sdk-runtime.mjs          write them
//   FUTURUNA_SDK_TS_DEPS=<dir> node scripts/sdk-runtime.mjs --check  fail unless current
//
// FUTURUNA_SDK_TS_DEPS names a directory whose node_modules holds `typescript`
// (5.x) and `zod`, as for tests/sdk_ts.test.mjs. Run it after every change to
// the runtime; tests/sdk_ts.test.mjs checks that the embedded files are current.
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const source = join(root, 'src/calculate/sdk_ts_runtime.ts');
export const runtimeJs = join(root, 'src/calculate/sdk_ts_runtime.js');
export const runtimeDts = join(root, 'src/calculate/sdk_ts_runtime.d.ts');

/** The runtime's source compiled with `deps`' TypeScript: `{ js, dts }`. */
export function compileRuntime(deps) {
  const dir = mkdtempSync(join(tmpdir(), 'futuruna-sdk-runtime-'));
  symlinkSync(join(deps, 'node_modules'), join(dir, 'node_modules'), 'dir');
  writeFileSync(join(dir, 'package.json'), '{ "type": "module" }\n');
  writeFileSync(join(dir, 'index.ts'), readFileSync(source, 'utf8'));
  const tsc = spawnSync(process.execPath, [
    join(deps, 'node_modules/typescript/bin/tsc'),
    '--strict', '--declaration', '--target', 'es2022', '--module', 'nodenext', '--moduleResolution', 'nodenext',
    '--exactOptionalPropertyTypes', '--noUncheckedIndexedAccess', '--outDir', 'out', 'index.ts',
  ], { cwd: dir, encoding: 'utf8' });
  if (tsc.error) throw tsc.error;
  if (tsc.status !== 0) throw new Error(`tsc failed on the runtime:\n${tsc.stdout}\n${tsc.stderr}`);
  return { js: readFileSync(join(dir, 'out/index.js'), 'utf8'), dts: readFileSync(join(dir, 'out/index.d.ts'), 'utf8') };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const deps = process.env.FUTURUNA_SDK_TS_DEPS;
  if (!deps) {
    console.error('Set FUTURUNA_SDK_TS_DEPS to a directory whose node_modules holds typescript and zod.');
    process.exit(2);
  }
  const { js, dts } = compileRuntime(resolve(deps));
  if (process.argv.includes('--check')) {
    const stale = [[runtimeJs, js], [runtimeDts, dts]].filter(([path, text]) => readFileSync(path, 'utf8') !== text);
    if (stale.length > 0) {
      console.error(`stale: ${stale.map(([path]) => path).join(', ')}; run node scripts/sdk-runtime.mjs`);
      process.exit(1);
    }
  } else {
    writeFileSync(runtimeJs, js);
    writeFileSync(runtimeDts, dts);
    console.log(`wrote ${runtimeJs} and ${runtimeDts}`);
  }
}
