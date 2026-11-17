// `runa sdk ts` end to end: generate an SDK, type-check it as strict
// TypeScript, and run its calls, validators and answering by a guide under
// Node, against the @futuruna/runtime package `runa sdk runtime` writes.
//
// FUTURUNA_SDK_TS_DEPS names a directory whose node_modules holds `typescript`
// (5.x) and `zod`; the test installs nothing. Where it also holds `eslint` (9.x)
// and `typescript-eslint` (8.x), the generated SDK and the runtime are linted
// with strictTypeChecked. With FUTURUNA_SDK_TS_SOURCES (a directory of
// *.entry.runa files) and FUTURUNA_SDK_TS_CASES (a directory of NAME.json
// input documents with NAME.expected.json outputs), the generated validators
// also check those cases and results, through a transport that answers from
// the expected outputs.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, symlinkSync, writeFileSync } from 'node:fs';
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
const fixture = 'tests/fixtures/calculation/tax.entry.runa';
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

/**
 * A project directory whose node_modules links every package of the deps and
 * holds @futuruna/runtime as `runa sdk runtime` writes it.
 */
function project(name) {
  const dir = mkdtempSync(join(tmpdir(), `futuruna-sdk-ts-${name}-`));
  const modules = join(dir, 'node_modules');
  mkdirSync(modules);
  for (const entry of readdirSync(join(deps, 'node_modules'))) {
    symlinkSync(join(deps, 'node_modules', entry), join(modules, entry), 'dir');
  }
  run(binary, ['sdk', 'runtime', '--out', join(modules, '@futuruna/runtime')]);
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
  calculateTaxSetAt, none,
  type CalculateTaxNeed, type PartialTaxInput, type TaxInput,
} from "./sdk/index.js";

export const sample: TaxInput = {
  monthlyIncome: 50000,
  filingStatus: "Married",
  deduction: null,
  children: [{ name: "Ada", age: 4 }],
};
export const started: PartialTaxInput = { children: [{ name: "Ada" }] };
export const noDeduction: PartialTaxInput = { ...started, deduction: none };
// @ts-expect-error a bare null is not "none" in a partial case
export const bareNull: PartialTaxInput = { deduction: null };

/** Each need's answer, typed by its pattern once the pattern is narrowed. */
export function answerNeed(partialCase: PartialTaxInput, need: CalculateTaxNeed): PartialTaxInput {
  switch (need.pattern) {
    case "children[].age":
      return calculateTaxSetAt(partialCase, need, 4);
    case "monthlyIncome":
      return calculateTaxSetAt(partialCase, need, 50000);
    case "filingStatus":
      return calculateTaxSetAt(partialCase, need, "Married");
    case "deduction":
      return calculateTaxSetAt(partialCase, need, none);
    default:
      return partialCase;
  }
}
// Type errors only; never called.
export function wrongAnswers(partialCase: PartialTaxInput, need: CalculateTaxNeed): void {
  // @ts-expect-error a need of several patterns takes no value until its pattern is narrowed
  calculateTaxSetAt(partialCase, need, 4);
  if (need.pattern === "children[].age") {
    // @ts-expect-error an age is a number
    calculateTaxSetAt(partialCase, need, "four");
  }
  if (need.pattern === "filingStatus") {
    // @ts-expect-error a filing status is one of its variants' names
    calculateTaxSetAt(partialCase, need, "Divorced");
  }
  if (need.pattern === "deduction") {
    // @ts-expect-error "none" is \`none\`, not null
    calculateTaxSetAt(partialCase, need, null);
  }
  // @ts-expect-error a need's path comes from the engine: a string is not an input path
  const forged: CalculateTaxNeed = { ...need, path: "children[-1].age" };
  void forged;
}
`;

/** The partial values and calls the reviews asked the types to check, on a small ledger model. */
const ledgerModel = `# Date(year: Int, month: Int, day: Int)
# Event = Deposit(amount: Int) | Withdrawal(amount: Int, reason: String?)
# Ledger(events: List(Event), closed_on: Date?, note: String?)
@ entry
> balance(input: Ledger) -> Int { 0 }
`;

const ledgerUsage = `import {
  balanceExplained, isoDate, none,
  type BalanceNeed, type EngineTransport, type ExplainingTransport, type IsoDate, type Ledger, type PartialEvent,
  type PartialLedger,
} from "./sdk/index.js";

function assertNever(value: never): never {
  throw new Error(JSON.stringify(value));
}

/** An exhaustive switch over a partial union handles \`{}\`, the variant not known yet. */
export function describe(event: PartialEvent): string {
  switch (event.$variant) {
    case "Deposit":
      return "in";
    case "Withdrawal":
      return "out";
    case undefined:
      return "not known yet";
    default:
      return assertNever(event);
  }
}
export function forgetsUnknown(event: PartialEvent): string {
  switch (event.$variant) {
    case "Deposit":
      return "in";
    case "Withdrawal":
      return "out";
    default:
      // @ts-expect-error \`{}\` is not handled
      return assertNever(event);
  }
}
export const unknownEvent: PartialEvent = {};
// @ts-expect-error junk is not an event whose variant is unknown
export const junk: PartialEvent = { amount: 1 };

export const closed: PartialLedger = { closedOn: none, note: none, events: [{}, { $variant: "Withdrawal", reason: none }] };
// @ts-expect-error a bare null is not "none" in a partial case
export const bareNull: PartialLedger = { note: null };
// @ts-expect-error neither is a bare null inside a variant
export const bareNullReason: PartialLedger = { events: [{ $variant: "Withdrawal", reason: null }] };

export const ledger: Ledger = { events: [], closedOn: isoDate("2026-03-01"), note: null };
// @ts-expect-error a bare string is not an IsoDate
export const looseDay: Ledger = { events: [], closedOn: "yesterday", note: null };
export const day: IsoDate | null = ledger.closedOn;

export const explain = (transport: ExplainingTransport) => balanceExplained(transport, ledger);
export function plainTransport(transport: EngineTransport) {
  // @ts-expect-error an explained call needs a transport that explains
  return balanceExplained(transport, ledger);
}

export function forgeNeed(need: BalanceNeed): BalanceNeed {
  // @ts-expect-error an input path comes from the engine, not from a string
  return { ...need, path: "events[-1].amount" };
}
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
        question: 'What is your income before tax each month?', waiting: [{ rule: 'annual_income' }] });
    }
    if (input.filing_status?.$variant === undefined) {
      needs.push({ kind: 'fact', path: 'filing_status.$variant', label: 'Filing status', waiting: [{ rule: 'annual_tax' }] });
    }
    (input.children ?? []).forEach((child, index) => {
      if (child.age === undefined) {
        needs.push({ kind: 'fact', path: `children[${index}].age`, label: 'Child age', unit: 'years', waiting: [{ rule: 'annual_tax' }] });
      }
    });
    needs.sort((left, right) => (left.path < right.path ? -1 : 1));
    const income = input.monthly_income * 12;
    const result = needs.length > 0
      ? { $variant: 'Needs', needs }
      : { $variant: 'Complete', result: { annual_income: income, taxable_income: income, annual_tax: income / 5 },
        assumptions: [{ section: 'tax', binding: 'flat_rate', value: { id: 'FLAT', resolve_with: 'the rate table' } }],
        unused_paths: ['children[0].name'] };
    return { $futuruna: { schema: 'futuruna.entry.partial-output.v1', schema_hash: hash, entry },
      results: [{ case_id: 'case-1', result }], diagnostics: [] };
  };
  return { calls, callPartial };
}

test('runa sdk ts output type-checks and calls and validates through a transport', options, async () => {
  const dir = project('fixture');
  run(binary, ['sdk', 'ts', fixture, '--out', join(dir, 'sdk')]);
  const again = project('fixture-again');
  run(binary, ['sdk', 'ts', fixture, '--out', join(again, 'sdk')]);
  assert.deepEqual(readdirSync(join(dir, 'sdk')).sort(), ['client.ts', 'index.ts', 'partial.ts', 'schema.ts', 'types.ts'],
    'no guide: no answer module, and the runtime is a package');
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
    { result: { annualIncome: 600000, taxableIncome: 600000, annualTax: 120000 }, assumptions: [] });

  // Ints beyond ±(2^53 − 1) are rejected before the engine runs, and in results.
  for (const monthlyIncome of [2 ** 53, -(2 ** 53), 2 ** 60, 1.5]) {
    await assert.rejects(sdk.calculateTax(cli, { ...usage.sample, monthlyIncome }),
      (error) => error.name === 'CalculationError' && error.kind === 'input');
  }
  await assert.rejects(sdk.calculateTax(cli, { ...usage.sample, filingStatus: { $variant: 'Married' } }),
    (error) => error.kind === 'input' && error.issues[0].path.join('.') === 'filingStatus');
  assert.equal(engineCalls, 1);
  await assert.rejects(sdk.calculateTax(cli, { ...usage.sample, surprise: 1 }), (error) => error.kind === 'input');
  await assert.rejects(sdk.calculateTax(cli, { ...usage.sample, monthly_income: 1 }), (error) => error.kind === 'input');
  const answering = (result, meta = {}, assumptions = undefined) => ({
    async call(entry) {
      return { $futuruna: { schema: 'futuruna.entry.output.v1', schema_hash: hash, entry, ...meta },
        results: result === undefined ? [] : [{ case_id: 'case-1', result, ...(assumptions && { assumptions }) }],
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
  await assert.rejects(sdk.calculateTax(answering({ annualIncome: 1, taxableIncome: 1, annualTax: 1 }), usage.sample),
    (error) => error.kind === 'output', 'results arrive in the engine\'s snake_case');
  assert.deepEqual(await sdk.calculateTax({ call: async () => ({ annual_income: 1, taxable_income: 1, annual_tax: 1 }) }, usage.sample),
    { result: { annualIncome: 1, taxableIncome: 1, annualTax: 1 }, assumptions: [] }, 'a bare result has no envelope to check');
  // A result carries the assumptions it rests on as the engine writes them; a malformed one is an output error.
  const flatRate = { section: 'tax', binding: 'flat_rate', value: { id: 'FLAT', resolve_with: 'the rate table' } };
  assert.deepEqual(await sdk.calculateTax(answering({ annual_income: 1, taxable_income: 1, annual_tax: 1 }, {}, [flatRate]), usage.sample),
    { result: { annualIncome: 1, taxableIncome: 1, annualTax: 1 }, assumptions: [flatRate] });
  await assert.rejects(sdk.calculateTax(answering({ annual_income: 1, taxable_income: 1, annual_tax: 1 }, {}, [{ binding: 'flat_rate' }]), usage.sample),
    (error) => error.kind === 'output');

  // The wire codec: snake_case fields, an enum as { $variant }, a date as { year, month, day }.
  const codec = sdk.calculateTaxEntry.input;
  assert.deepEqual(sdk.toWire(codec, usage.sample),
    { monthly_income: 50000, filing_status: { $variant: 'Married' }, deduction: null, children: [{ name: 'Ada', age: 4 }] });
  assert.deepEqual(sdk.fromWire(codec, sdk.toWire(codec, usage.sample)), usage.sample);
  assert.deepEqual(sdk.toWire(codec, { filingStatus: {}, children: [{}] }), { filing_status: {}, children: [{}] });
  // "none" is null on the wire; a field left out stays out.
  assert.deepEqual(sdk.toWire(codec, usage.noDeduction), { children: [{ name: 'Ada' }], deduction: null });
  assert.deepEqual(sdk.toWire(codec, { deduction: sdk.none }), { deduction: null });
  assert.equal(sdk.fromWirePath(codec, {}, 'filing_status.$variant'), 'filingStatus');
  assert.equal(sdk.fromWirePath(codec, { children: [{}] }, 'children[0].age'), 'children[0].age');
  assert.equal(sdk.fromWirePath(codec, {}, 'monthly_income'), 'monthlyIncome');
  assert.equal(sdk.fromWirePath(codec, {}, 'nowhere.deep'), 'nowhere.deep');
  assert.throws(() => sdk.fromWire(codec, { monthlyIncome: 1, children: [{ name: 'Ada', age_years: 4 }] }), (error) =>
    error.name === 'WireError' && error.issues.map((issue) => issue.path.join('.')).join() === 'monthlyIncome,children.0.age_years');
  assert.throws(() => sdk.toWire(codec, { monthly_income: 1 }), (error) => error.name === 'WireError');
  const dated = sdk.recordCodec({ bookedOn: ['booked_on', sdk.isoDateCodec], kind: ['kind', sdk.enumCodec] });
  assert.deepEqual(sdk.toWire(dated, { bookedOn: '2026-03-01', kind: 'Deposit' }),
    { booked_on: { year: 2026, month: 3, day: 1 }, kind: { $variant: 'Deposit' } });
  assert.deepEqual(sdk.fromWire(dated, { booked_on: { year: 26, month: 3, day: 1 } }), { bookedOn: '0026-03-01' });
  assert.equal(sdk.fromWirePath(dated, {}, 'booked_on.year'), 'bookedOn');
  for (const valid of ['2024-02-29', '2026-12-31', '0001-01-01']) {
    assert.ok(sdk.IsoDateSchema.safeParse(valid).success, valid);
    assert.equal(sdk.isoDate(valid), valid);
  }
  for (const invalid of ['2026-02-29', '2026-13-01', '2026-1-01', '2026-04-31', '20260301', 'yesterday']) {
    assert.ok(!sdk.IsoDateSchema.safeParse(invalid).success, invalid);
    assert.throws(() => sdk.isoDate(invalid), RangeError);
  }

  // setAt writes at concrete paths and never changes its argument.
  assert.deepEqual(sdk.setAt({ children: [{}, {}] }, 'children[1].age', 3), { children: [{}, { age: 3 }] });
  assert.deepEqual(sdk.setAt({ s: { $variant: 'A', x: 1 } }, 's.$variant', 'A'), { s: { $variant: 'A', x: 1 } });
  assert.deepEqual(sdk.setAt({ s: { $variant: 'A', x: 1 } }, 's.$variant', 'B'), { s: { $variant: 'B' } });
  assert.deepEqual(sdk.setAt({}, 'taxpayer.spouse.income', 1), { taxpayer: { spouse: { income: 1 } } });
  assert.deepEqual(usage.started, { children: [{ name: 'Ada' }] });
  assert.throws(() => sdk.setAt({ children: [] }, 'children[0].age', 1), (error) => error.name === 'PathError');
  assert.throws(() => sdk.setAt({ spouse: null }, 'spouse.income', 1), (error) => error.name === 'PathError');
  assert.throws(() => sdk.setAt({ spouse: sdk.none }, 'spouse.income', 1), (error) => error.name === 'PathError');
  assert.throws(() => sdk.parsePath('a..b'), (error) => error.name === 'PathError');
  assert.equal(sdk.pathPattern('accounts[12].events[0].kind'), 'accounts[].events[].kind');

  // Answering by a guide, through the runtime alone: a guide table written here, a partial engine.
  const engine = fixturePartialEngine(hash);
  const transport = { call: cli.call, callPartial: engine.callPartial };
  const query = (c) => sdk.calculateTaxQuery(transport, c);
  const guide = { name: 'fixture_guide', paths: { 'children[].age': 'Family', filingStatus: 'Family', monthlyIncome: 'Income' } };
  const first = await sdk.answerEntry(query, usage.started, guide, {
    Family: () => (need) => (need.pattern === 'children[].age' ? sdk.given(need, 4) : sdk.given(need, 'Married')),
    Income: sdk.leaveOpen,
  });
  assert.equal(first.$variant, 'Unanswered');
  assert.equal(first.rounds.length, 2);
  assert.deepEqual(Object.keys(first.open), ['Income'], 'the needs left open, keyed by the group that left them');
  assert.deepEqual(first.open.Income.map((need) => [need.pattern, need.path, need.by]), [['monthlyIncome', 'monthlyIncome', 'Income']]);
  assert.equal(first.open.Income[0].question, 'What is your income before tax each month?');
  assert.deepEqual(first.open.Income[0].waiting, [{ rule: 'annual_income' }]);
  assert.deepEqual(first.partialCase, { children: [{ name: 'Ada', age: 4 }], filingStatus: 'Married' });
  assert.ok(Array.isArray(first.later), 'Unanswered carries the last round\'s later facts');
  assert.deepEqual(first.pending, []);
  const second = await sdk.answerEntry(query, usage.answerNeed(first.partialCase, first.open.Income[0]), guide,
    { Family: sdk.leaveOpen, Income: sdk.leaveOpen });
  assert.equal(second.$variant, 'Answered');
  assert.deepEqual(second.result, { annualIncome: 600000, taxableIncome: 600000, annualTax: 120000 });
  assert.deepEqual(second.unusedPaths, ['children[0].name']);
  assert.deepEqual(second.assumptions, [flatRate], 'an Answered result carries its assumptions');
  assert.deepEqual(second.rounds, []);
  assert.equal(engine.calls.at(-1).$futuruna.schema_hash, hash);
  assert.deepEqual(engine.calls.at(-1).cases[0].input,
    { children: [{ name: 'Ada', age: 4 }], filing_status: { $variant: 'Married' }, monthly_income: 50000 });

  // Each group's resolver is called once per round with all of the round's needs of its group, and
  // the groups' resolvers run at the same time: each waits here until both have begun.
  const batches = [];
  let begun = 0;
  let allBegun;
  const together = new Promise((resolveTogether) => { allBegun = resolveTogether; });
  const batched = (group, value) => async (needs, context) => {
    batches.push([group, needs.map((need) => need.path), context.partialCase]);
    begun += 1;
    if (begun === 2) allBegun();
    await Promise.race([together, new Promise((_, reject) => {
      setTimeout(() => reject(new Error(`${group} waited for the other resolvers alone`)), 2000).unref();
    })]);
    return (need) => sdk.given(need, value(need));
  };
  const three = { children: [{ name: 'Ada' }, { name: 'Bo' }, { name: 'Cy' }] };
  const all = await sdk.answerEntry(query, three, guide, {
    Family: batched('Family', (need) => (need.pattern === 'filingStatus' ? 'Married' : 4)),
    Income: batched('Income', () => 50000),
  });
  assert.equal(all.$variant, 'Answered');
  assert.equal(all.rounds.length, 1);
  assert.deepEqual(batches.map(([group, paths]) => [group, paths]).sort(), [
    ['Family', ['children[0].age', 'children[1].age', 'children[2].age', 'filingStatus']],
    ['Income', ['monthlyIncome']],
  ]);
  assert.deepEqual(all.rounds[0].groups.Income.map((need) => need.by), ['Income']);
  assert.ok(batches.every(([, , partialCase]) => partialCase === three), 'every resolver sees the case of its round');
  assert.deepEqual(all.partialCase.children.map((child) => child.age), [4, 4, 4]);
  // A reply names the path it is for: one for another need is an error, not an answer in the wrong place.
  await assert.rejects(sdk.answerEntry(query, three, guide, { Family: (needs) => () => sdk.given(needs[0], 5), Income: sdk.leaveOpen }),
    (error) => error.name === 'AnswerError' && /replied to `children\[1\]\.age` with a reply for `children\[0\]\.age`/.test(error.message));
  // A need the guide names no group for is held: the held case should have had it.
  await assert.rejects(sdk.answerEntry(query, three, { name: 'no_income', paths: { 'children[].age': 'Family', filingStatus: 'Family' } },
    { Family: sdk.leaveOpen }),
  (error) => error.name === 'HeldMissingError' && error instanceof sdk.AnswerError && error.guide === 'no_income'
    && error.needs.map((need) => need.path).join() === 'monthlyIncome'
    && /^round 1: `monthlyIncome` is held \(the guide `no_income` names no group for it\)/.test(error.message));
  // Below a path the table has no key for (past a recursive type's cut), the nearest key above it decides.
  assert.equal(sdk.guideGroup({ name: 'g', paths: { 'tree.Branch.left': 'Shapes' } }, 'tree.Branch.left.Leaf.value'), 'Shapes');
  assert.equal(sdk.guideGroup({ name: 'g', paths: { 'tree.Branch.left': 'Shapes' } }, 'tree.Branch.right'), undefined);
  assert.equal(sdk.guidePath({ events: [{ $variant: 'Trade' }, { $variant: 'Dividend' }] },
    { pattern: 'events[].amount', path: 'events[1].amount' }), 'events[].Dividend.amount');
  assert.equal(sdk.guidePath({ events: [{}] }, { pattern: 'events[].$variant', path: 'events[0].$variant' }), 'events[].$variant');

  // Partial inputs and Needs results are validated too.
  await assert.rejects(sdk.calculateTaxQuery(transport, { children: [{ age: 2 ** 53 }] }), (error) => error.kind === 'input');
  await assert.rejects(sdk.calculateTaxQuery(transport, { deduction: null }), (error) => error.kind === 'input'
    && error.issues[0].path.join('.') === 'deduction', 'a bare null is not "none"');
  const needing = (needs, later) => ({
    async callPartial(entry) {
      return { $futuruna: { schema: 'futuruna.entry.partial-output.v1', schema_hash: hash, entry },
        results: [{ case_id: 'case-1', result: { $variant: 'Needs', needs, ...(later && { later }) } }], diagnostics: [] };
    },
  });
  const waiting = [{ rule: 'annual_tax' }];
  await assert.rejects(sdk.calculateTaxQuery(needing([{ kind: 'fact', path: 'nowhere', waiting }]), {}),
    (error) => error.kind === 'output');
  await assert.rejects(sdk.calculateTaxQuery(needing([{ kind: 'fact', path: 'monthly_income', waiting: [] }]), {}),
    (error) => error.kind === 'output', 'a need waits for at least one rule');
  await assert.rejects(sdk.calculateTaxQuery(needing([{ kind: 'fact', path: 'monthly_income', waiting,
    keys: [{ path: 'children[0]', field: 'name', value: 'Ada' }] }]), {}),
  (error) => error.kind === 'output', 'a key no list declares');
  const stuck = needing([{ kind: 'fact', path: 'children[0].age', waiting }]);
  await assert.rejects(sdk.answerEntry((c) => sdk.calculateTaxQuery(stuck, c), { children: [{}] }, guide,
    { Family: () => (need) => sdk.given(need, 1), Income: sdk.leaveOpen }),
  (error) => error.name === 'AnswerError' && /still needs `children\[0\]\.age`/.test(error.message));

  // A need is tagged with its pattern; a date is needed whole; a list element's key comes at TypeScript paths.
  const passing = { safeParse: (data) => ({ success: true, data }) };
  const positions = sdk.listCodec(sdk.recordCodec({ isin: ['isin', sdk.valueCodec], openedOn: ['opened_on', sdk.isoDateCodec],
    valueOre: ['value_ore', sdk.valueCodec] }));
  const dateEntry = { name: 'calculate_tax', schemaHash: hash, output: sdk.valueCodec,
    input: sdk.recordCodec({ bookedOn: ['booked_on', sdk.isoDateCodec], kind: ['kind', sdk.enumCodec],
      openingPositions: ['opening_positions', positions] }),
    patterns: ['bookedOn', 'openingPositions[]', 'openingPositions[].isin', 'openingPositions[].valueOre'] };
  const sources = [{ label: 'law', role: 'source', binding: 'b' }];
  const act = { title: 'aktiesparekontoloven', identifier: 'LBK nr. 281 af 2025' };
  const cited = [{ rule: 'r', section: 'ask', paragraphs: [{ paragraph: '§ 16', source: 'sources/01.txt', act }] }];
  const needsQuery = await sdk.queryEntry(needing([
    { kind: 'fact', path: 'booked_on', label: 'Booking date', sources, waiting: cited },
    { kind: 'fact', path: 'opening_positions[1].value_ore', waiting,
      keys: [{ path: 'opening_positions[1]', field: 'isin', value: 'DK0060534915' }] },
  ], [{ kind: 'fact', path: 'opening_positions[0].isin', waiting }]), dateEntry, passing, passing,
  { openingPositions: [{}, { isin: 'DK0060534915' }] });
  assert.deepEqual(needsQuery, { $variant: 'Needs', needs: [
    { kind: 'fact', path: 'bookedOn', pattern: 'bookedOn', label: 'Booking date',
      sources, waiting: cited },
    { kind: 'fact', path: 'openingPositions[1].valueOre', pattern: 'openingPositions[].valueOre', waiting,
      keys: [{ path: 'openingPositions[1]', field: 'isin', value: 'DK0060534915' }] },
  ], later: [{ kind: 'fact', path: 'openingPositions[0].isin', pattern: 'openingPositions[].isin', waiting }] });

  // Explanations at TypeScript paths.
  const explaining = {
    ...cli,
    async callExplained(entry, document, full) {
      const path = join(dir, `explain-${full}.json`);
      writeFileSync(path, JSON.stringify(document));
      return run(binary, ['call', fixture, '--entry', entry, '--input', path, full ? '--explain=full' : '--explain']);
    },
  };
  for (const depth of ['direct', 'full']) {
    const explained = await sdk.calculateTaxExplained(explaining, usage.sample, depth);
    assert.deepEqual(explained.result, { annualIncome: 600000, taxableIncome: 600000, annualTax: 120000 });
    assert.deepEqual(explained.assumptions, []);
    assert.deepEqual(Object.keys(explained.explanation.values), ['annualIncome', 'annualTax', 'taxableIncome']);
    const inputs = Object.values(explained.explanation.values).flatMap((value) => value.from ?? [])
      .filter((item) => item.$variant === 'Input').map((item) => item.path);
    assert.ok(inputs.includes('monthlyIncome'), JSON.stringify(explained.explanation));
    assert.equal(explained.explanation.derivation !== undefined, depth === 'full');
  }
  const by = (path) => ({ $variant: 'Input', path });
  const explanation = {
    sections: {},
    values: {
      'booked_on.year': { value: 2026, by: by('booked_on.year'), from_omitted: 2 },
      'booked_on.month': { value: 3, by: by('booked_on.month') },
      'booked_on.day': { value: 1, by: by('booked_on.day') },
      'paid_on.year': { value: 2026, by: by('booked_on.year') },
      'paid_on.month': { value: 4, by: { $variant: 'Computed' } },
      'paid_on.day': { value: 1, by: by('booked_on.day') },
      'kind.$variant': { value: 'Deposit', by: { $variant: 'Rule', rule: 'kind_of', clause: 'default' },
        from: [by('booked_on.year'), by('booked_on.month'), by('booked_on.day'), by('kind.$variant')]
          .map((item, at) => ({ ...item, value: [2026, 3, 1, 'Deposit'][at] })),
        because: [{ ...by('kind.$variant'), value: 'Deposit' }] },
    },
  };
  const explainedEntry = { ...dateEntry, output: sdk.recordCodec({ bookedOn: ['booked_on', sdk.isoDateCodec],
    paidOn: ['paid_on', sdk.isoDateCodec], kind: ['kind', sdk.enumCodec] }) };
  const document = { $futuruna: { schema: 'futuruna.entry.output.v1', schema_hash: hash, entry: 'calculate_tax' },
    results: [{ case_id: 'case-1', result: {}, assumptions: [flatRate], explanation }], diagnostics: [] };
  const decoded = await sdk.explainEntry({ callExplained: async () => document }, explainedEntry, 'direct', passing, passing, {});
  assert.deepEqual(decoded.assumptions, [flatRate]);
  assert.deepEqual(decoded.explanation.values, {
    bookedOn: { value: '2026-03-01', by: by('bookedOn'), fromOmitted: 2 },
    kind: { value: 'Deposit', by: { $variant: 'Rule', rule: 'kind_of', clause: 'default' },
      from: [{ $variant: 'Input', path: 'bookedOn', value: '2026-03-01' }, { $variant: 'Input', path: 'kind', value: 'Deposit' }],
      because: [{ $variant: 'Input', path: 'kind', value: 'Deposit' }] },
    'paidOn.day': { value: 1, by: by('bookedOn') },
    'paidOn.month': { value: 4, by: { $variant: 'Computed' } },
    'paidOn.year': { value: 2026, by: by('bookedOn') },
  });

  // An entry that needs itself for the same subject is a cycle: runEntry passes the stack on in `options`.
  const spouse = needing([{ kind: 'entry', path: 'deduction', entry: 'calculate_tax', subject: 'spouse', waiting }]);
  const spouseQuery = (c) => sdk.calculateTaxQuery(spouse, c);
  const byEntry = { name: 'by_entry', paths: { deduction: 'Spouse' } };
  const answerSpouse = (options) => sdk.answerEntry(spouseQuery, {}, byEntry, cyclic, options);
  const cyclic = { Spouse: (_needs, context) => (need) => context.runEntry(need, answerSpouse) };
  await assert.rejects(answerSpouse(), (error) => error.name === 'AnswerCycleError'
    && error.frames.length === 2 && error.frames.every((frame) => frame.subject === 'spouse'));
  // Another entry's Answered result is written at the need's path.
  const provided = { name: 'provided', paths: { deduction: 'Spouse' } };
  let asked = 0;
  const once = {
    async callPartial(entry, document) {
      asked += 1;
      if (document.cases[0].input.deduction === undefined) return spouse.callPartial(entry);
      return { $futuruna: { schema: 'futuruna.entry.partial-output.v1', schema_hash: hash, entry },
        results: [{ case_id: 'case-1', result: { $variant: 'Complete', result: { annual_income: 1, taxable_income: 1, annual_tax: 1 },
          unused_paths: [] } }],
        diagnostics: [] };
    },
  };
  const ran = await sdk.answerEntry((c) => sdk.calculateTaxQuery(once, c), {}, provided, {
    Spouse: (_needs, context) => (need) => context.runEntry(need, async (options) => {
      assert.deepEqual(options, { frames: [{ entry: 'calculate_tax', subject: 'spouse' }] });
      return { $variant: 'Answered', result: 700, assumptions: [], unusedPaths: [], partialCase: {}, rounds: [] };
    }),
  });
  assert.equal(ran.$variant, 'Answered');
  assert.deepEqual(ran.partialCase, { deduction: 700 });
  assert.equal(asked, 2);
});

test('generated SDKs validate a model\'s cases and expected results', modelOptions, async () => {
  const dir = project('model');
  const projectRoot = dirname(sources);
  const models = readdirSync(sources).filter((name) => name.endsWith('.entry.runa')).sort();
  for (const model of models) {
    const out = join(dir, model.replace(/\.entry\.runa$/, ''));
    run(binary, ['sdk', 'ts', relative(projectRoot, join(sources, model)), '--out', out], projectRoot);
  }
  const indexes = models.map((model) => `${model.replace(/\.entry\.runa$/, '')}/index.ts`);
  compile(dir, indexes);
  const sdks = await Promise.all(models.map((model) =>
    import(pathToFileURL(join(dir, 'build', model.replace(/\.entry\.runa$/, ''), 'index.js')))));

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
      // The cases are runa call documents: decode them, and check that encoding gives them back.
      const codecs = sdk[`${valueName(entry)}Entry`];
      const tsInput = sdk.fromWire(codecs.input, inputCase.input);
      assert.deepEqual(sdk.toWire(codecs.input, tsInput), inputCase.input, `${inputFile} ${inputCase.case_id} round trip`);
      const call = sdk[valueName(entry)](transport, tsInput);
      if (result) {
        const calculated = await call;
        assert.deepEqual(sdk.toWire(codecs.output, calculated.result), result.result, `${inputFile} ${inputCase.case_id}`);
        assert.deepEqual(calculated.assumptions, result.assumptions ?? [], `${inputFile} ${inputCase.case_id} assumptions`);
        console.log(`${inputFile} ${inputCase.case_id}: input and ${expectedFile} result valid (${entry})`);
      } else {
        await assert.rejects(call, (error) => error.kind === 'diagnostics');
        console.log(`${inputFile} ${inputCase.case_id}: input valid, ${diagnostics.length} expected diagnostic(s)`);
      }
    }
  }
});

const unitModel = `# unit Ore "DKK øre" = DKK / 100
# unit Micro "millionths of a share" = 1 / 1000000
# unit Third "thirds of a krone" = DKK / 3
# Holding(quantity_micro: Micro, value_ore: Ore, count: Int, history: List(Ore), share: Third)
@ entry
> holding_value(input: Holding) -> Ore { input.value_ore }
`;

const unitUsage = `import {
  holdingValue, holdingValueSetAt, micro, ore, third, OreSchema,
  type EngineTransport, type Holding, type HoldingValueNeed, type Micro, type Ore, type PartialHolding,
} from "./sdk/index.js";

export const holding: Holding = {
  quantityMicro: micro(2_000_000), valueOre: ore.fromDecimal("12.00"), count: 3, history: [ore(1100)], share: third(1),
};
const shares: Micro = holding.quantityMicro;
// @ts-expect-error a Micro is not an Ore
export const wrongUnit: Holding = { ...holding, valueOre: shares };
// @ts-expect-error a bare number is not an Ore
export const bare: Holding = { ...holding, valueOre: 1200 };
// @ts-expect-error a bare number is not an Ore in a list either
export const bareItem: Holding = { ...holding, history: [1100] };
// @ts-expect-error a Micro converted from a decimal is still a Micro
export const convertedWrong: Holding = { ...holding, valueOre: micro.fromDecimal("1.5") };
// @ts-expect-error a decimal amount is text, never a float
export const floated: Ore = ore.fromDecimal(12.5);
export const plain: Holding = { ...holding, count: 4 };
export const started: PartialHolding = {};
export function answerNeed(partialCase: PartialHolding, need: HoldingValueNeed): PartialHolding {
  if (need.pattern === "valueOre") return holdingValueSetAt(partialCase, need, ore.fromDecimal("0.05"));
  if (need.pattern === "history[]") {
    // @ts-expect-error a Micro is not an Ore at an Ore path
    holdingValueSetAt(partialCase, need, shares);
    // @ts-expect-error setAt takes the path's unit
    holdingValueSetAt(partialCase, need, 5);
  }
  return partialCase;
}
export const parsed: Ore = OreSchema.parse(1200);
/** How the integrator reads a figure in kroner. */
export const inKroner = async (transport: EngineTransport): Promise<string> =>
  ore.toDecimal((await holdingValue(transport, holding)).result);
`;

test('a unit of measure is a branded number type, and its scale gives exact decimal converters', options, async () => {
  const dir = project('units');
  writeFileSync(join(dir, 'holding.entry.runa'), unitModel);
  run(binary, ['sdk', 'ts', 'holding.entry.runa', '--out', 'sdk'], dir);
  writeFileSync(join(dir, 'usage.ts'), unitUsage);
  compile(dir, ['sdk/index.ts', 'usage.ts']);
  const sdk = await import(pathToFileURL(join(dir, 'build/sdk/index.js')));
  assert.equal(sdk.ore(1200), 1200);
  assert.throws(() => sdk.ore(1.5), RangeError);
  assert.throws(() => sdk.micro(2 ** 53), RangeError);
  assert.ok(!sdk.OreSchema.safeParse(0.5).success);

  // Decimal amounts convert exactly, in integer arithmetic.
  assert.equal(sdk.ore.fromDecimal('1740.25'), 174025);
  assert.equal(sdk.ore.fromDecimal('1740.2'), 174020);
  assert.equal(sdk.ore.fromDecimal('1740.250000'), 174025, 'trailing zeros are no finer amount');
  assert.equal(sdk.ore.fromDecimal('-0.05'), -5);
  assert.equal(sdk.ore.fromDecimal('+3'), 300);
  assert.equal(sdk.ore.fromDecimal('0.29'), 29, 'no float rounding: 0.29 * 100 is 28.999999999999996 as a float');
  assert.equal(sdk.ore.fromDecimal('90071992547409.91'), Number.MAX_SAFE_INTEGER);
  assert.equal(sdk.micro.fromDecimal('2.5'), 2500000);
  assert.equal(sdk.ore.toDecimal(sdk.ore(174025)), '1740.25');
  assert.equal(sdk.ore.toDecimal(sdk.ore(174000)), '1740.00');
  assert.equal(sdk.ore.toDecimal(sdk.ore(-5)), '-0.05');
  assert.equal(sdk.ore.toDecimal(sdk.ore(Number.MAX_SAFE_INTEGER)), '90071992547409.91');
  assert.equal(sdk.micro.toDecimal(sdk.micro(2500000)), '2.500000');
  for (const text of ['0.29', '1740.25', '-12.07', '0.00']) {
    assert.equal(sdk.ore.toDecimal(sdk.ore.fromDecimal(text)), text.replace(/^-0\.00$/, '0.00'));
  }
  // An amount finer than the unit is refused, naming the unit and the amount.
  assert.throws(() => sdk.ore.fromDecimal('1740.255'), (error) => {
    assert.equal(error.name, 'CalculationError');
    assert.equal(error.kind, 'amount');
    assert.equal(error.entry, 'Ore');
    assert.equal(error.message, 'Ore: 1740.255 DKK is finer than one Ore (1/100 DKK)');
    return true;
  });
  assert.throws(() => sdk.micro.fromDecimal('0.0000005'), (error) => error.kind === 'amount'
    && error.message === 'Micro: 0.0000005 is finer than one Micro (1/1000000)');
  for (const text of ['1,5', '1.', '.5', '', '1e3', ' 1']) {
    assert.throws(() => sdk.ore.fromDecimal(text), (error) => error.kind === 'amount' && /is not a decimal number of DKK/.test(error.message), text);
  }
  assert.throws(() => sdk.ore.fromDecimal('90071992547409.92'), (error) => error.kind === 'amount'
    && /beyond a safe integer of Ore/.test(error.message));
  assert.equal(sdk.third.fromDecimal, undefined, '1/3 DKK is no finite decimal: no converters');

  const cli = {
    async call(entry, document) {
      writeFileSync(join(dir, 'call.json'), JSON.stringify(document));
      return run(binary, ['call', 'holding.entry.runa', '--entry', entry, '--input', 'call.json'], dir);
    },
    async callPartial() { throw new Error('not used'); },
  };
  const usage = await import(pathToFileURL(join(dir, 'build/usage.js')));
  assert.equal(await usage.inKroner(cli), '12.00');
});

const scalarModel = `| isin_ok(text: String) -> string_length(text) == 12
# Isin: String "an ISIN" where isin_ok
# AccountId: String "an account id"
# Holding(isin: Isin, account: AccountId, count: Int)
# Book(holdings: List(Holding))
@ entry
> first_isin(input: Book) -> Isin { head(input.holdings).isin }
`;

const scalarUsage = `import {
  firstIsin, isin, accountId, IsinSchema,
  type Book, type EngineTransport, type Isin, type AccountId,
} from "./sdk/index.js";

const account: AccountId = accountId("A-1");
export const good: Book = { holdings: [{ isin: isin("DK0060534915"), account, count: 1 }] };
// @ts-expect-error a bare string is not an Isin
export const bare: Book = { holdings: [{ isin: "DK0060534915", account, count: 1 }] };
// @ts-expect-error an AccountId is not an Isin
export const swapped: Book = { holdings: [{ isin: account, account, count: 1 }] };
export const asText: string = isin("DK0060534915");
export const parsed: Isin = IsinSchema.parse("DK0060534915");
export const bad: Book = {
  holdings: [good.holdings[0]!, { isin: isin("DK006053491"), account, count: 2 }],
};
export const result = async (transport: EngineTransport, book: Book): Promise<Isin> => (await firstIsin(transport, book)).result;
`;

test('a named scalar is a branded type in the SDK, and a value the engine refuses is an input error at its path', options, async () => {
  const dir = project('scalars');
  writeFileSync(join(dir, 'book.entry.runa'), scalarModel);
  run(binary, ['sdk', 'ts', 'book.entry.runa', '--out', 'sdk'], dir);
  writeFileSync(join(dir, 'usage.ts'), scalarUsage);
  compile(dir, ['sdk/index.ts', 'usage.ts']);
  const sdk = await import(pathToFileURL(join(dir, 'build/sdk/index.js')));
  assert.equal(sdk.isin('DK0060534915'), 'DK0060534915');
  assert.throws(() => sdk.isin(12), RangeError);
  assert.ok(!sdk.IsinSchema.safeParse(12).success);
  const cli = {
    async call(entry, document) {
      writeFileSync(join(dir, 'call.json'), JSON.stringify(document));
      // The engine exits non-zero when it refuses a case; its reply is still the document.
      const p = spawnSync(binary, ['call', 'book.entry.runa', '--entry', entry, '--input', 'call.json'], {
        cwd: dir, encoding: 'utf8', timeout: 600000,
      });
      assert.ifError(p.error);
      return p.stdout;
    },
    async callPartial() { throw new Error('not used'); },
  };
  const usage = await import(pathToFileURL(join(dir, 'build/usage.js')));
  assert.equal(await usage.result(cli, usage.good), 'DK0060534915');
  await assert.rejects(usage.result(cli, usage.bad), (error) => {
    assert.equal(error.name, 'CalculationError');
    assert.equal(error.kind, 'input');
    assert.equal(error.scalar, 'Isin');
    assert.equal(error.check, 'isin_ok');
    assert.deepEqual(error.issues.map((issue) => issue.path), ['holdings[1].isin']);
    return true;
  });
});

test('partial values, dates, paths and explained calls are checked by the types', options, async () => {
  const dir = project('ledger');
  writeFileSync(join(dir, 'ledger.entry.runa'), ledgerModel);
  run(binary, ['sdk', 'ts', 'ledger.entry.runa', '--out', 'sdk'], dir);
  writeFileSync(join(dir, 'usage.ts'), ledgerUsage);
  compile(dir, ['sdk/index.ts', 'usage.ts']);
  const sdk = await import(pathToFileURL(join(dir, 'build/sdk/index.js')));
  const usage = await import(pathToFileURL(join(dir, 'build/usage.js')));
  assert.equal(usage.describe({}), 'not known yet');
  assert.deepEqual(sdk.toWire(sdk.balanceEntry.input, usage.closed),
    { closed_on: null, note: null, events: [{}, { $variant: 'Withdrawal', reason: null }] });
  assert.ok(sdk.PartialLedgerSchema.safeParse(usage.closed).success);
  assert.ok(!sdk.PartialLedgerSchema.safeParse({ note: null }).success, 'a bare null is not "none"');
  assert.ok(sdk.LedgerSchema.safeParse(usage.ledger).success);
});

/** Events keyed by their id in every variant, for second-order resolution. */
const keyedModel = `# Event =
    Deposit(@ field(label = "Event id", key = True) id: String, @ field(label = "Amount") amount: Int) |
    Withdrawal(@ field(label = "Event id", key = True) id: String, @ field(label = "Amount") amount: Int, @ field(label = "Fee") fee: Int)
# Ledger(events: List(Event))
> signed(e: Event) -> Int {
    match e {
        | Deposit -> e.amount
        | Withdrawal -> 0 - e.amount - e.fee
    }
}
@ entry
> balance(input: Ledger) -> Int { foldl(input.events, 0, |sum, e| sum + signed(e)) }
`;

/** Who gives the events: the list as skeletons first, then the fields the law reads. */
const keyedGuide = `@ import ./keyed.entry
# LedgerSource = Skeletons | Details
= ledger_guide = AnswerGuide(fields = [
    EntryField(path = pathof(Ledger::events), by = Skeletons),
    EntryField(path = pathof(Ledger::events::Deposit::amount), by = Details),
    EntryField(path = pathof(Ledger::events::Withdrawal::amount), by = Details),
    EntryField(path = pathof(Ledger::events::Withdrawal::fee), by = Details)
])
`;

const keyedUsage = `import {
  balanceAnswer, given, open,
  type BalanceElementKey, type BalanceNeed, type BalanceResolvers, type EngineTransport, type PartialEvent,
} from "./sdk/index.js";

export type Row = { readonly amount: number; readonly fee: number };
/** A backend that answers in batches: the year's event skeletons, then the details of many events at once. */
export interface Backend {
  skeletons(): Promise<PartialEvent[]>;
  details(ids: readonly string[]): Promise<ReadonlyMap<string, Row>>;
}

/** The id of the event a need lies in, from its element key. */
export function eventId(need: BalanceNeed): string {
  const key: BalanceElementKey | undefined = need.keys?.find((candidate) => candidate.field === "id");
  if (key === undefined) throw new Error(\`\${need.path} lies in no keyed event\`);
  const id: string = key.value;
  return id;
}

export function resolvers(backend: Backend): BalanceResolvers {
  return {
    // The list, as skeletons: each event's variant and id, its details left out.
    Skeletons: async () => {
      const skeletons = await backend.skeletons();
      return (need) => (need.pattern === "events" ? given(need, skeletons) : open);
    },
    // Every field the law reads of the events, from one lookup of their ids. The switch is
    // exhaustive: the group's needs are of its patterns only.
    Details: async (needs) => {
      const rows = await backend.details([...new Set(needs.map(eventId))]);
      return (need) => {
        const row = rows.get(eventId(need));
        if (row === undefined) return open;
        switch (need.pattern) {
          case "events[].amount":
            return given(need, row.amount);
          case "events[].fee":
            return given(need, row.fee);
        }
      };
    },
  };
}

export const answerLedger = (transport: EngineTransport, backend: Backend) => balanceAnswer(transport, {}, resolvers(backend));
`;

/** A transport that runs `runa call --partial` on `model` in `dir`, keeping each partial case it sends. */
function partialCli(dir, model) {
  const queried = [];
  return {
    queried,
    async call() { throw new Error('not used'); },
    async callPartial(entry, document) {
      const path = join(dir, `partial-${queried.length}.json`);
      queried.push(document.cases[0].input);
      writeFileSync(path, JSON.stringify(document));
      return run(binary, ['call', model, '--entry', entry, '--input', path, '--partial'], dir);
    },
  };
}

test('second-order resolution by group: a list of skeletons, then the fields the law reads, by key', options, async () => {
  const dir = project('keyed');
  writeFileSync(join(dir, 'keyed.entry.runa'), keyedModel);
  writeFileSync(join(dir, 'keyed.guide.runa'), keyedGuide);
  run(binary, ['sdk', 'ts', 'keyed.entry.runa', '--out', 'sdk', '--guides', 'keyed.guide.runa', '--guide', 'balance=ledger_guide'], dir);
  writeFileSync(join(dir, 'usage.ts'), keyedUsage);
  compile(dir, ['sdk/index.ts', 'usage.ts']);
  const sdk = await import(pathToFileURL(join(dir, 'build/sdk/index.js')));
  const usage = await import(pathToFileURL(join(dir, 'build/usage.js')));
  assert.match(readFileSync(join(dir, 'sdk/partial.ts'), 'utf8'),
    /export type BalanceElementKey =\n {2}\| ElementKey<"events\[\]", "id", string>;/);

  const transport = partialCli(dir, 'keyed.entry.runa');
  const skeletons = [{ $variant: 'Deposit', id: 'e1' }, { $variant: 'Withdrawal', id: 'e2' }, { $variant: 'Deposit', id: 'e3' }];

  // The needs inside a skeleton carry its event's id.
  const round2 = await sdk.balanceQuery(transport, { events: skeletons });
  assert.equal(round2.$variant, 'Needs');
  assert.deepEqual(round2.needs.map((need) => [need.path, need.pattern, need.keys]), [
    ['events[0].amount', 'events[].amount', [{ path: 'events[0]', field: 'id', value: 'e1' }]],
    ['events[1].amount', 'events[].amount', [{ path: 'events[1]', field: 'id', value: 'e2' }]],
    ['events[1].fee', 'events[].fee', [{ path: 'events[1]', field: 'id', value: 'e2' }]],
    ['events[2].amount', 'events[].amount', [{ path: 'events[2]', field: 'id', value: 'e3' }]],
  ]);

  // Round 1 asks Skeletons for the list, round 2 Details for every field of the events by their ids:
  // one backend call per round.
  transport.queried.length = 0;
  const rows = new Map([['e1', { amount: 100, fee: 0 }], ['e2', { amount: 30, fee: 2 }], ['e3', { amount: 7, fee: 0 }]]);
  const backend = { skeletons: 0, details: [] };
  const answering = await usage.answerLedger(transport, {
    async skeletons() {
      backend.skeletons += 1;
      return skeletons;
    },
    async details(ids) {
      backend.details.push([...ids]);
      return new Map(ids.map((id) => [id, rows.get(id)]));
    },
  });
  assert.equal(answering.$variant, 'Answered', JSON.stringify(answering));
  assert.equal(answering.result, 100 - 30 - 2 + 7);
  assert.deepEqual(answering.rounds.map((round) => Object.fromEntries(Object.entries(round.groups)
    .map(([group, needs]) => [group, needs.map((need) => need.path)]))), [
    { Skeletons: ['events'] },
    { Details: ['events[0].amount', 'events[1].amount', 'events[1].fee', 'events[2].amount'] },
  ]);
  assert.deepEqual(transport.queried, [{}, { events: skeletons }, { events: [
    { $variant: 'Deposit', id: 'e1', amount: 100 }, { $variant: 'Withdrawal', id: 'e2', amount: 30, fee: 2 },
    { $variant: 'Deposit', id: 'e3', amount: 7 }] }]);
  assert.equal(backend.skeletons, 1);
  assert.deepEqual(backend.details, [['e1', 'e2', 'e3']]);
});

/** The household of tests/expect/run/needs/answer_household.runa, answered from TypeScript. */
const householdModel = `# Spouse(income: Int, cohabiting: Bool)
# Child(name: String, age: Int)
# Household(income: Int, married: Bool, spouse: Option(Spouse), children: List(Child))
# Result(tax: Int)

| threshold(h: Household) -> match h.spouse {
    | Some(s) -> if s.cohabiting { 200 - s.income } else { 100 }
    | None -> 100
}

| child_allowance(h: Household) -> foldl(map(h.children, |c| if c.age < 18 { 10 } else { 0 }), 0, |sum, x| sum + x)

@ entry("Household tax")
| household_tax(h: Household) -> Result(tax = h.income - threshold(h) - child_allowance(h))
`;

const householdGuide = `@ import ./household.entry

-- The registry has the children's ages; the customer is asked about a spouse.
-- Everything else is in the integrator's own records: held.
# Origin = Registry | Customer
= household_guide = AnswerGuide(fields = [
    EntryField(path = pathof(Household::spouse), by = Customer),
    EntryField(path = pathof(Household::children::age), by = Registry)
])
`;

const householdUsage = `import {
  given, householdTaxAnswer, leaveOpen, open,
  type EngineTransport, type HouseholdTaxGuidedNeed, type HouseholdTaxHeld, type HouseholdTaxResolvers,
} from "./sdk/index.js";

/** Each resolver call: its group and the paths of its needs. */
export const calls: [string, string[]][] = [];

export const held: HouseholdTaxHeld = { income: 500, married: true, children: [{ name: "Ada" }, { name: "Bo" }] };
// @ts-expect-error a held fact left out does not compile: \`income\` is held
export const forgotIncome: HouseholdTaxHeld = { married: true, children: [] };
// @ts-expect-error a child's name is held too
export const namelessChild: HouseholdTaxHeld = { income: 500, married: true, children: [{ age: 4 }] };
/** A held fact given as \`undefined\`, not known: the type cannot see it, answering can. */
export const unknownIncome: HouseholdTaxHeld = { ...held, income: undefined };
/** A resolved fact may be given when known. */
export const knownSpouse: HouseholdTaxHeld = { ...held, spouse: { income: 50, cohabiting: true } };

const ages: ReadonlyMap<string, number> = new Map([["children[0].age", 4], ["children[1].age", 19]]);

export const registry: HouseholdTaxResolvers["Registry"] = (needs) => {
  calls.push(["Registry", needs.map((need) => need.path)]);
  return (need) => {
    const age = ages.get(need.path);
    return age === undefined ? open : given(need, age);
  };
};

export const customer: HouseholdTaxResolvers["Customer"] = (needs) => {
  calls.push(["Customer", needs.map((need) => need.path)]);
  return (need) => {
    switch (need.pattern) {
      case "spouse":
        return given(need, {});
      case "spouse.income":
        return given(need, 50);
      case "spouse.cohabiting":
        return given(need, true);
    }
  };
};

export const resolvers: HouseholdTaxResolvers = { Registry: registry, Customer: customer };
/** Nobody can give the ages yet: one visible line. */
export const registryClosed: HouseholdTaxResolvers = { Registry: leaveOpen, Customer: customer };
// @ts-expect-error every group's resolver is required
export const missingGroup: HouseholdTaxResolvers = { Registry: registry };
export const wrongAge: HouseholdTaxResolvers = {
  // @ts-expect-error an age is a number
  Registry: () => (need) => given(need, "four"),
  Customer: leaveOpen,
};
export const outOfGroup: HouseholdTaxResolvers = {
  Registry: () => (need) => {
    // @ts-expect-error the registry is never asked about the spouse
    if (need.pattern === "spouse") return open;
    return open;
  },
  Customer: leaveOpen,
};
/** Narrowing \`by\` narrows the need's pattern. */
export function describe(need: HouseholdTaxGuidedNeed): string {
  return need.by === "Registry" ? \`ask the registry for \${need.pattern}\` : \`ask the customer: \${need.question ?? need.label ?? need.pattern}\`;
}

export const answerHousehold = (transport: EngineTransport, heldCase: HouseholdTaxHeld, chosen: HouseholdTaxResolvers) =>
  householdTaxAnswer(transport, heldCase, chosen);

/** The paths each group left open, keyed by group. */
export async function openPaths(transport: EngineTransport): Promise<{ registry: string[]; customer: string[] }> {
  const answering = await householdTaxAnswer(transport, held, registryClosed);
  if (answering.$variant === "Answered") return { registry: [], customer: [] };
  return {
    registry: (answering.open.Registry ?? []).map((need) => need.path),
    customer: (answering.open.Customer ?? []).map((need) => need.path),
  };
}
`;

test('answering by a guide: the held case, resolvers batched once per group per round, open needs by group', options, async () => {
  const dir = project('household');
  writeFileSync(join(dir, 'household.entry.runa'), householdModel);
  writeFileSync(join(dir, 'household.guide.runa'), householdGuide);
  run(binary, ['sdk', 'ts', 'household.entry.runa', '--out', 'sdk',
    '--guides', 'household.guide.runa', '--guide', 'household_tax=household_guide'], dir);
  assert.ok(existsSync(join(dir, 'sdk/answer.ts')));
  writeFileSync(join(dir, 'usage.ts'), householdUsage);
  compile(dir, ['sdk/index.ts', 'usage.ts']);
  const sdk = await import(pathToFileURL(join(dir, 'build/sdk/index.js')));
  const usage = await import(pathToFileURL(join(dir, 'build/usage.js')));
  assert.deepEqual(sdk.householdTaxGuide, { name: 'household_guide', paths: {
    'children[].age': 'Registry', spouse: 'Customer', 'spouse.cohabiting': 'Customer', 'spouse.income': 'Customer',
  } });
  const byRound = (answering) => answering.rounds.map((round) => Object.fromEntries(Object.entries(round.groups)
    .map(([group, needs]) => [group, needs.map((need) => [need.path, need.by])])));

  // Each group's resolver is called once per round with all of that round's needs of its group.
  const transport = partialCli(dir, 'household.entry.runa');
  const answered = await usage.answerHousehold(transport, usage.held, usage.resolvers);
  assert.equal(answered.$variant, 'Answered', JSON.stringify(answered));
  assert.deepEqual(answered.result, { tax: 500 - (200 - 50) - 10 });
  assert.deepEqual(byRound(answered), [
    { Registry: [['children[0].age', 'Registry'], ['children[1].age', 'Registry']], Customer: [['spouse', 'Customer']] },
    { Customer: [['spouse.cohabiting', 'Customer'], ['spouse.income', 'Customer']] },
  ]);
  assert.deepEqual(usage.calls.filter(([group]) => group === 'Registry'), [['Registry', ['children[0].age', 'children[1].age']]]);
  assert.deepEqual(usage.calls.filter(([group]) => group === 'Customer'),
    [['Customer', ['spouse']], ['Customer', ['spouse.cohabiting', 'spouse.income']]]);
  assert.deepEqual(answered.partialCase, { ...usage.held, children: [{ name: 'Ada', age: 4 }, { name: 'Bo', age: 19 }],
    spouse: { cohabiting: true, income: 50 } });
  assert.equal(transport.queried.length, 3);
  assert.equal(usage.describe(answered.rounds[0].groups.Registry[0]), 'ask the registry for children[].age');

  // A group whose resolver is leaveOpen: the others answer, then answering ends with its needs open.
  const closed = await usage.answerHousehold(transport, usage.held, usage.registryClosed);
  assert.equal(closed.$variant, 'Unanswered');
  assert.equal(closed.rounds.length, 3);
  assert.deepEqual(Object.keys(closed.open), ['Registry']);
  assert.deepEqual(closed.open.Registry.map((need) => [need.path, need.by]),
    [['children[0].age', 'Registry'], ['children[1].age', 'Registry']]);
  assert.deepEqual(closed.partialCase.spouse, { cohabiting: true, income: 50 });
  assert.deepEqual(await usage.openPaths(transport), { registry: ['children[0].age', 'children[1].age'], customer: [] });

  // A resolved fact the held case gives is not asked.
  usage.calls.length = 0;
  const known = await usage.answerHousehold(transport, usage.knownSpouse, usage.resolvers);
  assert.equal(known.$variant, 'Answered');
  assert.deepEqual(usage.calls, [['Registry', ['children[0].age', 'children[1].age']]]);

  // A held fact the case leaves unknown is the integrator's data defect, before any resolver runs.
  usage.calls.length = 0;
  await assert.rejects(usage.answerHousehold(transport, usage.unknownIncome, usage.resolvers), (error) => {
    assert.ok(error instanceof sdk.HeldMissingError);
    assert.equal(error.guide, 'household_guide');
    assert.deepEqual(error.needs.map((need) => need.path), ['income']);
    assert.match(error.message, /^round 1: `income` is held \(the guide `household_guide` names no group for it\), but the held case leaves it unknown$/);
    return true;
  });
  assert.deepEqual(usage.calls, []);

  // Regenerating without the guide leaves the plain call and query, and no answer module.
  run(binary, ['sdk', 'ts', 'household.entry.runa', '--out', 'sdk'], dir);
  assert.ok(!existsSync(join(dir, 'sdk/answer.ts')));
  assert.ok(!readFileSync(join(dir, 'sdk/index.ts'), 'utf8').includes('answer.js'));
});

test('the runtime package is the compilation of the runtime source, at Futuruna\'s version', options, async () => {
  const { compileRuntime, runtimeJs, runtimeDts } = await import(pathToFileURL(join(root, 'scripts/sdk-runtime.mjs')));
  const { js, dts } = compileRuntime(deps);
  assert.equal(readFileSync(runtimeJs, 'utf8'), js, 'stale src/calculate/sdk_ts_runtime.js: run node scripts/sdk-runtime.mjs');
  assert.equal(readFileSync(runtimeDts, 'utf8'), dts, 'stale src/calculate/sdk_ts_runtime.d.ts: run node scripts/sdk-runtime.mjs');

  const dir = project('runtime');
  const runtimeDir = join(dir, 'node_modules/@futuruna/runtime');
  const version = run(binary, ['--version']).trim().split(/\s+/).at(-1);
  const manifest = JSON.parse(readFileSync(join(runtimeDir, 'package.json'), 'utf8'));
  assert.equal(manifest.name, '@futuruna/runtime');
  assert.equal(manifest.version, version);
  assert.deepEqual(manifest.files, ['index.js', 'index.d.ts']);
  const runtime = await import(pathToFileURL(join(runtimeDir, 'index.js')));
  assert.match(runtime.INPUT_SCHEMA, /^futuruna\.entry\.input\.v\d+$/);
  assert.equal(typeof runtime.answerEntry, 'function');
  assert.equal(runtime.resolve, undefined);
  // npm packs it as the release asset futuruna-runtime-<version>.tgz.
  const pack = spawnSync('npm', ['pack', '--dry-run', '--json'], { cwd: runtimeDir, encoding: 'utf8' });
  assert.ifError(pack.error);
  assert.equal(pack.status, 0, pack.stderr);
  const [packed] = JSON.parse(pack.stdout);
  assert.equal(packed.filename, `futuruna-runtime-${version}.tgz`);
  assert.deepEqual(packed.files.map((file) => file.path).sort(), ['index.d.ts', 'index.js', 'package.json']);
});

test('the generated SDK and the runtime pass typescript-eslint strictTypeChecked', {
  skip: !(ready && existsSync(join(deps, 'node_modules/typescript-eslint')))
    && 'Set FUTURUNA_SDK_TS_DEPS to node_modules that also hold eslint and typescript-eslint.',
}, () => {
  const dir = project('lint');
  run(binary, ['sdk', 'ts', fixture, '--out', join(dir, 'sdk/tax')]);
  writeFileSync(join(dir, 'ledger.entry.runa'), ledgerModel);
  run(binary, ['sdk', 'ts', 'ledger.entry.runa', '--out', 'sdk/ledger'], dir);
  writeFileSync(join(dir, 'household.entry.runa'), householdModel);
  writeFileSync(join(dir, 'household.guide.runa'), householdGuide);
  run(binary, ['sdk', 'ts', 'household.entry.runa', '--out', 'sdk/household',
    '--guides', 'household.guide.runa', '--guide', 'household_tax=household_guide'], dir);
  writeFileSync(join(dir, 'holding.entry.runa'), unitModel);
  run(binary, ['sdk', 'ts', 'holding.entry.runa', '--out', 'sdk/holding'], dir);
  mkdirSync(join(dir, 'runtime'));
  copyFileSync(join(root, 'src/calculate/sdk_ts_runtime.ts'), join(dir, 'runtime/index.ts'));
  writeFileSync(join(dir, 'tsconfig.json'), JSON.stringify({
    compilerOptions: { target: 'es2023', module: 'nodenext', moduleResolution: 'nodenext', strict: true,
      noUncheckedIndexedAccess: true, exactOptionalPropertyTypes: true, noEmit: true, types: [] },
    include: ['sdk/**/*.ts', 'runtime/**/*.ts'],
  }));
  writeFileSync(join(dir, 'eslint.config.mjs'), `import tseslint from "typescript-eslint";
export default tseslint.config(...tseslint.configs.strictTypeChecked, {
  languageOptions: { parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname } },
});
`);
  run(process.execPath, [join(deps, 'node_modules/eslint/bin/eslint.js'), '--max-warnings', '0', 'sdk', 'runtime'], dir);
});
