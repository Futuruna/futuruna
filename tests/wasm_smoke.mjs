import fs from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";

function reviveBigInts(value) {
  if (Array.isArray(value)) {
    return value.map(reviveBigInts);
  }
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, inner]) => [key, reviveBigInts(inner)]),
    );
  }
  if (typeof value === "string" && /^-?\d+n$/.test(value)) {
    return BigInt(value.slice(0, -1));
  }
  return value;
}

function stableRepr(value) {
  return JSON.stringify(value, (_key, inner) =>
    typeof inner === "bigint" ? `${inner.toString()}n` : inner,
  );
}

async function main() {
  const [pkgDir, exportName, argsJson, expectedJson] = process.argv.slice(2);
  if (!pkgDir || !exportName || argsJson === undefined || expectedJson === undefined) {
    throw new Error(
      "usage: node tests/wasm_smoke.mjs <pkg-dir> <export-name> <args-json> <expected-json>",
    );
  }

  const entries = await fs.readdir(pkgDir);
  const jsFile = entries.find((name) => name.endsWith(".js") && !name.endsWith(".d.ts"));
  const wasmFile = entries.find((name) => name.endsWith(".wasm"));
  if (!jsFile || !wasmFile) {
    throw new Error(`expected .js and .wasm output in ${pkgDir}`);
  }

  const mod = await import(pathToFileURL(path.join(pkgDir, jsFile)).href);
  const init = mod.default;
  if (typeof init !== "function") {
    throw new Error(`expected default init() export in ${jsFile}`);
  }
  if (typeof mod[exportName] !== "function") {
    throw new Error(`expected named export ${exportName} in ${jsFile}`);
  }

  const wasmBytes = await fs.readFile(path.join(pkgDir, wasmFile));
  await init({ module_or_path: wasmBytes });

  const args = reviveBigInts(JSON.parse(argsJson));
  const expected = reviveBigInts(JSON.parse(expectedJson));
  const actual = mod[exportName](...args);

  if (stableRepr(actual) !== stableRepr(expected)) {
    throw new Error(
      `unexpected result for ${exportName}: got ${stableRepr(actual)}, expected ${stableRepr(expected)}`,
    );
  }

  console.log(`smoke ok: ${exportName} -> ${stableRepr(actual)}`);
}

main().catch((err) => {
  console.error(err.stack || String(err));
  process.exit(1);
});
