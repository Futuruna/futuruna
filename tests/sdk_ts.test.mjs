// `runa sdk ts` end to end: generate an SDK, type-check it as strict
// TypeScript, and run its calls, validators and resolver loop under Node.
//
// FUTURUNA_SDK_TS_DEPS names a directory whose node_modules holds `typescript`
// (5.x) and `zod`; the test installs nothing. With FUTURUNA_SDK_TS_SOURCES (a
// directory of *.calculate.runa files) and FUTURUNA_SDK_TS_CASES (a directory
// of NAME.json input documents with NAME.expected.json outputs), the generated
// validators also check those cases and results, through a transport that
// answers from the expected outputs.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, readdirSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import test from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
const deps = process.env.FUTURUNA_SDK_TS_DEPS && resolve(process.env.FUTURUNA_SDK_TS_DEPS);
const sources = process.env.FUTURUNA_SDK_TS_SOURCES && resolve(process.env.FUTURUNA_SDK_TS_SOURCES);
const casesDir = process.env.FUTURUNA_SDK_TS_CASES && resolve(process.env.FUTURUNA_SDK_TS_CASES);
const ready = binary && deps;
const options = {
  skip: !ready && 'Set FUTURUNA_MODEL_TEST_RUNA and FUTURUNA_SDK_TS_DEPS (node_modules with typescript and zod).',
};
const modelOptions = {
  skip: !(ready && sources && casesDir) && 'Set FUTURUNA_SDK_TS_SOURCES and FUTURUNA_SDK_TS_CASES as well.',
};
const fixture = 'tests/fixtures/calculation/tax.calculate.runa';
const compilerFlags = ['--strict', '--target', 'es2022', '--module', 'nodenext', '--moduleResolution', 'nodenext'];

function run(command, args, cwd = root) {
  const p = spawnSync(command, args, {
    cwd, encoding: 'utf8', timeout: 600000, maxBuffer: 64 * 1024 * 1024,
    env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' },
  });
  assert.ifError(p.error);
  assert.equal(p.status, 0, `${command} ${args.join(' ')}\n${p.stdout}\n${p.stderr}`);
  return p.stdout;
}

function project(name) {
  const dir = mkdtempSync(join(tmpdir(), `futuruna-sdk-ts-${name}-`));
  symlinkSync(join(deps, 'node_modules'), join(dir, 'node_modules'), 'dir');
  writeFileSync(join(dir, 'package.json'), '{ "type": "module" }\n');
  return dir;
}

/** Type-check strictly (also with the strictest optional flags), then emit JavaScript into build/. */
function compile(dir, files) {
  const tsc = (args) => run(process.execPath, [join(deps, 'node_modules/typescript/bin/tsc'), ...args], dir);
  tsc(['--noEmit', ...compilerFlags, ...files]);
  tsc(['--noEmit', ...compilerFlags, '--exactOptionalPropertyTypes', '--noUncheckedIndexedAccess', ...files]);
  tsc([...compilerFlags, '--rootDir', '.', '--outDir', 'build', ...files]);
}

const valueName = (entry) => {
  const pascal = entry.split('_').filter(Boolean).map((part) => part[0].toUpperCase() + part.slice(1)).join('');
  return pascal[0].toLowerCase() + pascal.slice(1);
};

const fixtureUsage = `import {
  answer, ask, calculateTaxQuery, calculateTaxSetAt, resolve,
  type CalculateTaxResolvers, type EngineTransport, type PartialTaxInput, type TaxInput,
} from "./sdk/index.js";

export const sample: TaxInput = {
  monthly_income: 50000,
  filing_status: { $variant: "Married" },
  deduction: null,
  children: [{ name: "Ada", age: 4 }],
};
export const started: PartialTaxInput = { children: [{ name: "Ada" }] };
export const answered: PartialTaxInput = calculateTaxSetAt(started, "children[0].age", 4);
// @ts-expect-error the contract has no such path
calculateTaxSetAt(started, "children[0].height", 4);
// @ts-expect-error an age is a number
calculateTaxSetAt(started, "children[0].age", "four");
export const resolvers: CalculateTaxResolvers = {
  "children[].age": () => answer(4),
  "filing_status.$variant": () => answer("Married" as const),
  monthly_income: () => ask,
};
export const interview = (transport: EngineTransport, partialCase: PartialTaxInput) =>
  resolve((c: PartialTaxInput) => calculateTaxQuery(transport, c), partialCase, resolvers);
`;

/** A partial engine for the fixture: it needs the income, the filing status and each child's age. */
function fixturePartialEngine(hash) {
  const calls = [];
  const callPartial = async (entry, document) => {
    calls.push(document);
    const input = document.cases[0].input;
    const needs = [];
    if (input.monthly_income === undefined) {
      needs.push({ kind: 'fact', path: 'monthly_income', label: 'Monthly income',
        question: 'What is your income before tax each month?', waiting_rule: 'annual_income' });
    }
    if (input.filing_status?.$variant === undefined) {
      needs.push({ kind: 'fact', path: 'filing_status.$variant', label: 'Filing status', waiting_rule: 'annual_tax' });
    }
    (input.children ?? []).forEach((child, index) => {
      if (child.age === undefined) {
        needs.push({ kind: 'fact', path: `children[${index}].age`, label: 'Child age', unit: 'years', waiting_rule: 'annual_tax' });
      }
    });
    needs.sort((left, right) => (left.path < right.path ? -1 : 1));
    const income = input.monthly_income * 12;
    const result = needs.length > 0
      ? { $variant: 'Needs', needs }
      : { $variant: 'Complete', result: { annual_income: income, taxable_income: income, annual_tax: income / 5 },
        unused_paths: ['children[0].name'] };
    return { $futuruna: { schema: 'futuruna.calculate.partial-output.v1', schema_hash: hash, entry },
      results: [{ case_id: 'case-1', result }], diagnostics: [] };
  };
  return { calls, callPartial };
}

test('runa sdk ts output type-checks and calls, validates and resolves through a transport', options, async () => {
  const dir = project('fixture');
  run(binary, ['sdk', 'ts', fixture, '--out', join(dir, 'sdk')]);
  const again = project('fixture-again');
  run(binary, ['sdk', 'ts', fixture, '--out', join(again, 'sdk')]);
  for (const name of ['types.ts', 'schema.ts', 'partial.ts', 'client.ts', 'index.ts']) {
    assert.equal(readFileSync(join(dir, 'sdk', name), 'utf8'), readFileSync(join(again, 'sdk', name), 'utf8'), name);
  }
  writeFileSync(join(dir, 'usage.ts'), fixtureUsage);
  compile(dir, ['sdk/index.ts', 'usage.ts']);
  const sdk = await import(pathToFileURL(join(dir, 'build/sdk/index.js')));
  const usage = await import(pathToFileURL(join(dir, 'build/usage.js')));
  const hash = sdk.SCHEMA_HASH.calculate_tax;

  // A complete call through the real `runa call`.
  let engineCalls = 0;
  const cli = {
    async call(entry, document) {
      engineCalls += 1;
      const path = join(dir, `call-${engineCalls}.json`);
      writeFileSync(path, JSON.stringify(document));
      return run(binary, ['call', fixture, '--entry', entry, '--input', path]);
    },
    async callPartial() { throw new Error('not used'); },
  };
  assert.deepEqual(await sdk.calculateTax(cli, usage.sample),
    { annual_income: 600000, taxable_income: 600000, annual_tax: 120000 });

  // Ints beyond ±(2^53 − 1) are rejected before the engine runs, and in results.
  for (const monthly_income of [2 ** 53, -(2 ** 53), 2 ** 60, 1.5]) {
    await assert.rejects(sdk.calculateTax(cli, { ...usage.sample, monthly_income }),
      (error) => error.name === 'CalculationError' && error.kind === 'input');
  }
  assert.equal(engineCalls, 1);
  await assert.rejects(sdk.calculateTax(cli, { ...usage.sample, surprise: 1 }), (error) => error.kind === 'input');
  const answering = (result, meta = {}) => ({
    async call(entry) {
      return { $futuruna: { schema: 'futuruna.calculate.output.v1', schema_hash: hash, entry, ...meta },
        results: result === undefined ? [] : [{ case_id: 'case-1', result }],
        diagnostics: result === undefined ? [{ case_id: 'case-1', path: '$.cases[0].input', message: 'boom' }] : [] };
    },
    async callPartial() { throw new Error('not used'); },
  });
  await assert.rejects(sdk.calculateTax(answering({ annual_income: 2 ** 53, taxable_income: 0, annual_tax: 0 }), usage.sample),
    (error) => error.kind === 'output');
  await assert.rejects(sdk.calculateTax(answering({ annual_income: 1, taxable_income: 1, annual_tax: 1 }, { schema_hash: 'other' }), usage.sample),
    (error) => error.kind === 'envelope' && /this SDK was generated for/.test(error.message));
  await assert.rejects(sdk.calculateTax(answering(undefined), usage.sample),
    (error) => error.kind === 'diagnostics' && error.diagnostics[0].message === 'boom');
  assert.deepEqual(await sdk.calculateTax({ call: async () => ({ annual_income: 1, taxable_income: 1, annual_tax: 1 }) }, usage.sample),
    { annual_income: 1, taxable_income: 1, annual_tax: 1 }, 'a bare result has no envelope to check');

  // setAt writes at concrete paths and never changes its argument.
  assert.deepEqual(sdk.setAt({ children: [{}, {}] }, 'children[1].age', 3), { children: [{}, { age: 3 }] });
  assert.deepEqual(sdk.setAt({ s: { $variant: 'A', x: 1 } }, 's.$variant', 'A'), { s: { $variant: 'A', x: 1 } });
  assert.deepEqual(sdk.setAt({ s: { $variant: 'A', x: 1 } }, 's.$variant', 'B'), { s: { $variant: 'B' } });
  assert.deepEqual(sdk.setAt({}, 'taxpayer.spouse.income', 1), { taxpayer: { spouse: { income: 1 } } });
  assert.deepEqual(usage.started, { children: [{ name: 'Ada' }] });
  assert.throws(() => sdk.setAt({ children: [] }, 'children[0].age', 1), (error) => error.name === 'PathError');
  assert.throws(() => sdk.setAt({ spouse: null }, 'spouse.income', 1), (error) => error.name === 'PathError');
  assert.throws(() => sdk.parsePath('a..b'), (error) => error.name === 'PathError');
  assert.equal(sdk.pathPattern('accounts[12].events[0].kind'), 'accounts[].events[].kind');

  // The resolver loop: answers in rounds, stops at questions, then completes.
  const engine = fixturePartialEngine(hash);
  const transport = { call: cli.call, callPartial: engine.callPartial };
  const first = await usage.interview(transport, usage.started);
  assert.equal(first.$variant, 'Questions');
  assert.equal(first.rounds, 2);
  assert.deepEqual(first.questions.map((need) => need.path), ['monthly_income']);
  assert.equal(first.questions[0].question, 'What is your income before tax each month?');
  assert.deepEqual(first.partialCase, { children: [{ name: 'Ada', age: 4 }], filing_status: { $variant: 'Married' } });
  const second = await usage.interview(transport, sdk.calculateTaxSetAt(first.partialCase, 'monthly_income', 50000));
  assert.equal(second.$variant, 'Complete');
  assert.deepEqual(second.result, { annual_income: 600000, taxable_income: 600000, annual_tax: 120000 });
  assert.deepEqual(second.unused_paths, ['children[0].name']);
  assert.equal(engine.calls.at(-1).$futuruna.schema_hash, hash);

  // Partial inputs and Needs results are validated too.
  await assert.rejects(sdk.calculateTaxQuery(transport, { children: [{ age: 2 ** 53 }] }), (error) => error.kind === 'input');
  const needing = (needs) => ({
    async callPartial(entry) {
      return { $futuruna: { schema: 'futuruna.calculate.partial-output.v1', schema_hash: hash, entry },
        results: [{ case_id: 'case-1', result: { $variant: 'Needs', needs } }], diagnostics: [] };
    },
  });
  await assert.rejects(sdk.calculateTaxQuery(needing([{ kind: 'fact', path: 'nowhere', waiting_rule: 'r' }]), {}),
    (error) => error.kind === 'output');
  const stuck = needing([{ kind: 'fact', path: 'children[0].age', waiting_rule: 'annual_tax' }]);
  await assert.rejects(sdk.resolve((c) => sdk.calculateTaxQuery(stuck, c), { children: [{}] }, { 'children[].age': () => sdk.answer(1) }),
    (error) => error.name === 'ResolveError' && /still needs `children\[0\]\.age`/.test(error.message));

  // An entry that needs itself for the same subject is a cycle.
  const spouse = needing([{ kind: 'entry', path: 'deduction', entry: 'calculate_tax', subject: 'spouse', waiting_rule: 'taxable_income' }]);
  const query = (c) => sdk.calculateTaxQuery(spouse, c);
  const resolvers = { deduction: (_need, context) => context.runEntry(query, {}, resolvers) };
  await assert.rejects(sdk.resolve(query, {}, resolvers), (error) => error.name === 'ResolveCycleError'
    && error.frames.length === 2 && error.frames.every((frame) => frame.subject === 'spouse'));
});

test('generated SDKs validate a model\'s cases and expected results', modelOptions, async () => {
  const dir = project('model');
  const projectRoot = dirname(sources);
  const models = readdirSync(sources).filter((name) => name.endsWith('.calculate.runa')).sort();
  for (const model of models) {
    const out = join(dir, model.replace(/\.calculate\.runa$/, ''));
    run(binary, ['sdk', 'ts', relative(projectRoot, join(sources, model)), '--out', out], projectRoot);
  }
  const indexes = models.map((model) => `${model.replace(/\.calculate\.runa$/, '')}/index.ts`);
  compile(dir, indexes);
  const sdks = await Promise.all(models.map((model) =>
    import(pathToFileURL(join(dir, 'build', model.replace(/\.calculate\.runa$/, ''), 'index.js')))));

  const expectedFiles = readdirSync(casesDir).filter((name) => name.endsWith('.expected.json')).sort();
  assert.ok(expectedFiles.length > 0, `no *.expected.json in ${casesDir}`);
  for (const expectedFile of expectedFiles) {
    const inputFile = expectedFile.replace(/\.expected\.json$/, '.json');
    const input = JSON.parse(readFileSync(join(casesDir, inputFile), 'utf8'));
    const expected = JSON.parse(readFileSync(join(casesDir, expectedFile), 'utf8'));
    const { entry, schema_hash: hash } = input.$futuruna;
    const sdk = sdks.find((candidate) => candidate.SCHEMA_HASH[entry] === hash);
    assert.ok(sdk, `${inputFile}: no generated SDK has entry ${entry} with schema hash ${hash}`);
    for (const inputCase of input.cases) {
      const result = expected.results.find((item) => item.case_id === inputCase.case_id);
      const diagnostics = expected.diagnostics.filter((item) => item.case_id === inputCase.case_id);
      const transport = {
        async call(calledEntry, document) {
          assert.equal(calledEntry, entry);
          assert.equal(document.$futuruna.schema_hash, hash);
          return { $futuruna: expected.$futuruna, results: result ? [result] : [], diagnostics };
        },
        async callPartial() { throw new Error('not used'); },
      };
      const call = sdk[valueName(entry)](transport, inputCase.input);
      if (result) {
        assert.deepEqual(await call, result.result, `${inputFile} ${inputCase.case_id}`);
        console.log(`${inputFile} ${inputCase.case_id}: input and ${expectedFile} result valid (${entry})`);
      } else {
        await assert.rejects(call, (error) => error.kind === 'diagnostics');
        console.log(`${inputFile} ${inputCase.case_id}: input valid, ${diagnostics.length} expected diagnostic(s)`);
      }
    }
  }
});
