// `runa sdk ts` end to end: generate an SDK, type-check it as strict
// TypeScript, and run its calls, validators and resolver loop under Node.
//
// FUTURUNA_SDK_TS_DEPS names a directory whose node_modules holds `typescript`
// (5.x) and `zod`; the test installs nothing. Where it also holds `eslint` (9.x)
// and `typescript-eslint` (8.x), the generated SDK is linted with
// strictTypeChecked. With FUTURUNA_SDK_TS_SOURCES (a directory of
// *.calculate.runa files) and FUTURUNA_SDK_TS_CASES (a directory of NAME.json
// input documents with NAME.expected.json outputs), the generated validators
// also check those cases and results, through a transport that answers from
// the expected outputs.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, readdirSync, symlinkSync, writeFileSync } from 'node:fs';
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
  answer, ask, calculateTaxQuery, calculateTaxSetAt, none, resolve,
  type CalculateTaxNeed, type CalculateTaxResolvers, type EngineTransport, type PartialTaxInput, type TaxInput,
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
export const resolvers: CalculateTaxResolvers = {
  "children[].age": () => () => answer(4),
  filingStatus: () => () => answer("Married" as const),
  monthlyIncome: () => () => ask,
};
// @ts-expect-error resolvers are keyed by pattern, not by concrete path
export const concrete: CalculateTaxResolvers = { "children[0].age": () => () => answer(4) };
// @ts-expect-error a resolver answers each need with a function, not with a list that could be in another order
export const listed: CalculateTaxResolvers = { "children[].age": (needs) => needs.map(() => answer(4)) };
// @ts-expect-error an age is a number
export const wrongAge: CalculateTaxResolvers = { "children[].age": () => () => answer("four") };
export const interview = (transport: EngineTransport, partialCase: PartialTaxInput) =>
  resolve((c: PartialTaxInput) => calculateTaxQuery(transport, c), partialCase, resolvers);
`;

/** The partial values and calls the reviews asked the types to check, on a small ledger model. */
const ledgerModel = `# Date(year: Int, month: Int, day: Int)
# Event = Deposit(amount: Int) | Withdrawal(amount: Int, reason: String?)
# Ledger(events: List(Event), closed_on: Date?, note: String?)
@ calculate
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
  for (const name of ['types.ts', 'schema.ts', 'partial.ts', 'client.ts', 'index.ts', 'runtime.ts']) {
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
    { annualIncome: 600000, taxableIncome: 600000, annualTax: 120000 });

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
  await assert.rejects(sdk.calculateTax(answering({ annualIncome: 1, taxableIncome: 1, annualTax: 1 }), usage.sample),
    (error) => error.kind === 'output', 'results arrive in the engine\'s snake_case');
  assert.deepEqual(await sdk.calculateTax({ call: async () => ({ annual_income: 1, taxable_income: 1, annual_tax: 1 }) }, usage.sample),
    { annualIncome: 1, taxableIncome: 1, annualTax: 1 }, 'a bare result has no envelope to check');

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

  // The resolver loop: answers in rounds, stops at questions, then completes.
  const engine = fixturePartialEngine(hash);
  const transport = { call: cli.call, callPartial: engine.callPartial };
  const first = await usage.interview(transport, usage.started);
  assert.equal(first.$variant, 'Questions');
  assert.equal(first.rounds, 2);
  assert.deepEqual(first.questions.map((need) => [need.pattern, need.path]), [['monthlyIncome', 'monthlyIncome']]);
  assert.equal(first.questions[0].question, 'What is your income before tax each month?');
  assert.deepEqual(first.questions[0].waiting, [{ rule: 'annual_income' }]);
  assert.deepEqual(first.partialCase, { children: [{ name: 'Ada', age: 4 }], filingStatus: 'Married' });
  const second = await usage.interview(transport, usage.answerNeed(first.partialCase, first.questions[0]));
  assert.equal(second.$variant, 'Complete');
  assert.deepEqual(second.result, { annualIncome: 600000, taxableIncome: 600000, annualTax: 120000 });
  assert.deepEqual(second.unusedPaths, ['children[0].name']);
  assert.equal(engine.calls.at(-1).$futuruna.schema_hash, hash);
  assert.deepEqual(engine.calls.at(-1).cases[0].input,
    { children: [{ name: 'Ada', age: 4 }], filing_status: { $variant: 'Married' }, monthly_income: 50000 });

  // A resolver is called once per round with all of the round's needs of its pattern, and the
  // resolvers of different patterns run at the same time: each waits here until all three have begun.
  const batches = [];
  let begun = 0;
  let allBegun;
  const together = new Promise((resolveTogether) => { allBegun = resolveTogether; });
  const batched = (pattern, value) => async (needs, context) => {
    batches.push([pattern, needs.map((need) => need.path), context.partialCase]);
    begun += 1;
    if (begun === 3) allBegun();
    await Promise.race([together, new Promise((_, reject) => {
      setTimeout(() => reject(new Error(`${pattern} waited for the other resolvers alone`)), 2000).unref();
    })]);
    return () => sdk.answer(value);
  };
  const three = { children: [{ name: 'Ada' }, { name: 'Bo' }, { name: 'Cy' }] };
  const all = await sdk.resolve((c) => sdk.calculateTaxQuery(transport, c), three, {
    'children[].age': batched('children[].age', 4),
    filingStatus: batched('filingStatus', 'Married'),
    monthlyIncome: batched('monthlyIncome', 50000),
  });
  assert.equal(all.$variant, 'Complete');
  assert.equal(all.rounds, 2);
  assert.deepEqual(batches.map(([pattern, paths]) => [pattern, paths]).sort(), [
    ['children[].age', ['children[0].age', 'children[1].age', 'children[2].age']],
    ['filingStatus', ['filingStatus']],
    ['monthlyIncome', ['monthlyIncome']],
  ]);
  assert.ok(batches.every(([, , partialCase]) => partialCase === three), 'every resolver sees the case of its round');
  assert.deepEqual(all.partialCase.children.map((child) => child.age), [4, 4, 4]);

  // Partial inputs and Needs results are validated too.
  await assert.rejects(sdk.calculateTaxQuery(transport, { children: [{ age: 2 ** 53 }] }), (error) => error.kind === 'input');
  await assert.rejects(sdk.calculateTaxQuery(transport, { deduction: null }), (error) => error.kind === 'input'
    && error.issues[0].path.join('.') === 'deduction', 'a bare null is not "none"');
  const needing = (needs, later) => ({
    async callPartial(entry) {
      return { $futuruna: { schema: 'futuruna.calculate.partial-output.v1', schema_hash: hash, entry },
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
  await assert.rejects(sdk.resolve((c) => sdk.calculateTaxQuery(stuck, c), { children: [{}] }, { 'children[].age': () => () => sdk.answer(1) }),
    (error) => error.name === 'ResolveError' && /still needs `children\[0\]\.age`/.test(error.message));

  // A need is tagged with its pattern; a date is needed whole; a list element's key comes at TypeScript paths.
  const passing = { safeParse: (data) => ({ success: true, data }) };
  const positions = sdk.listCodec(sdk.recordCodec({ isin: ['isin', sdk.valueCodec], openedOn: ['opened_on', sdk.isoDateCodec],
    valueOre: ['value_ore', sdk.valueCodec] }));
  const dateEntry = { name: 'calculate_tax', schemaHash: hash, output: sdk.valueCodec,
    input: sdk.recordCodec({ bookedOn: ['booked_on', sdk.isoDateCodec], kind: ['kind', sdk.enumCodec],
      openingPositions: ['opening_positions', positions] }),
    patterns: ['bookedOn', 'openingPositions[]', 'openingPositions[].isin', 'openingPositions[].valueOre'] };
  const sources = [{ label: 'law', role: 'source', binding: 'b', attachment_path: 'law.pdf' }];
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
      sources: [{ label: 'law', role: 'source', binding: 'b', attachmentPath: 'law.pdf' }], waiting: cited },
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
  const document = { $futuruna: { schema: 'futuruna.calculate.output.v1', schema_hash: hash, entry: 'calculate_tax' },
    results: [{ case_id: 'case-1', result: {}, explanation }], diagnostics: [] };
  const decoded = await sdk.explainEntry({ callExplained: async () => document }, explainedEntry, 'direct', passing, passing, {});
  assert.deepEqual(decoded.explanation.values, {
    bookedOn: { value: '2026-03-01', by: by('bookedOn'), fromOmitted: 2 },
    kind: { value: 'Deposit', by: { $variant: 'Rule', rule: 'kind_of', clause: 'default' },
      from: [{ $variant: 'Input', path: 'bookedOn', value: '2026-03-01' }, { $variant: 'Input', path: 'kind', value: 'Deposit' }],
      because: [{ $variant: 'Input', path: 'kind', value: 'Deposit' }] },
    'paidOn.day': { value: 1, by: by('bookedOn') },
    'paidOn.month': { value: 4, by: { $variant: 'Computed' } },
    'paidOn.year': { value: 2026, by: by('bookedOn') },
  });

  // An entry that needs itself for the same subject is a cycle.
  const spouse = needing([{ kind: 'entry', path: 'deduction', entry: 'calculate_tax', subject: 'spouse', waiting }]);
  const query = (c) => sdk.calculateTaxQuery(spouse, c);
  const resolvers = { deduction: (_needs, context) => (need) => context.runEntry(query, {}, resolvers, need) };
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
      // The cases are runa call documents: decode them, and check that encoding gives them back.
      const codecs = sdk[`${valueName(entry)}Entry`];
      const tsInput = sdk.fromWire(codecs.input, inputCase.input);
      assert.deepEqual(sdk.toWire(codecs.input, tsInput), inputCase.input, `${inputFile} ${inputCase.case_id} round trip`);
      const call = sdk[valueName(entry)](transport, tsInput);
      if (result) {
        assert.deepEqual(sdk.toWire(codecs.output, await call), result.result, `${inputFile} ${inputCase.case_id}`);
        console.log(`${inputFile} ${inputCase.case_id}: input and ${expectedFile} result valid (${entry})`);
      } else {
        await assert.rejects(call, (error) => error.kind === 'diagnostics');
        console.log(`${inputFile} ${inputCase.case_id}: input valid, ${diagnostics.length} expected diagnostic(s)`);
      }
    }
  }
});

const unitModel = `# unit Ore "DKK øre"
# unit Micro "millionths of a share"
# Holding(quantity_micro: Micro, value_ore: Ore, count: Int, history: List(Ore))
@ calculate
> holding_value(input: Holding) -> Ore { input.value_ore }
`;

const unitUsage = `import {
  holdingValue, holdingValueSetAt, micro, ore, OreSchema, resolve, answer, holdingValueQuery,
  type EngineTransport, type Holding, type HoldingValueNeed, type HoldingValueResolvers, type Micro, type Ore,
  type PartialHolding,
} from "./sdk/index.js";

export const holding: Holding = { quantityMicro: micro(2_000_000), valueOre: ore(1200), count: 3, history: [ore(1100)] };
const shares: Micro = holding.quantityMicro;
// @ts-expect-error a Micro is not an Ore
export const wrongUnit: Holding = { ...holding, valueOre: shares };
// @ts-expect-error a bare number is not an Ore
export const bare: Holding = { ...holding, valueOre: 1200 };
// @ts-expect-error a bare number is not an Ore in a list either
export const bareItem: Holding = { ...holding, history: [1100] };
export const plain: Holding = { ...holding, count: 4 };
export const started: PartialHolding = {};
export function answerNeed(partialCase: PartialHolding, need: HoldingValueNeed): PartialHolding {
  if (need.pattern === "valueOre") return holdingValueSetAt(partialCase, need, ore(5));
  if (need.pattern === "history[]") {
    // @ts-expect-error a Micro is not an Ore at an Ore path
    holdingValueSetAt(partialCase, need, shares);
    // @ts-expect-error setAt takes the path's unit
    holdingValueSetAt(partialCase, need, 5);
  }
  return partialCase;
}
export const resolvers: HoldingValueResolvers = { valueOre: () => () => answer(ore(7)), quantityMicro: () => () => answer(micro(1)) };
export const parsed: Ore = OreSchema.parse(1200);
export const result = (transport: EngineTransport): Promise<Ore> => holdingValue(transport, holding);
export const interview = (transport: EngineTransport) =>
  resolve((c: PartialHolding) => holdingValueQuery(transport, c), started, resolvers);
`;

test('a unit of measure is a branded number type in the SDK', options, async () => {
  const dir = project('units');
  writeFileSync(join(dir, 'holding.calculate.runa'), unitModel);
  run(binary, ['sdk', 'ts', 'holding.calculate.runa', '--out', 'sdk'], dir);
  writeFileSync(join(dir, 'usage.ts'), unitUsage);
  compile(dir, ['sdk/index.ts', 'usage.ts']);
  const sdk = await import(pathToFileURL(join(dir, 'build/sdk/index.js')));
  assert.equal(sdk.ore(1200), 1200);
  assert.throws(() => sdk.ore(1.5), RangeError);
  assert.throws(() => sdk.micro(2 ** 53), RangeError);
  assert.ok(!sdk.OreSchema.safeParse(0.5).success);
  const cli = {
    async call(entry, document) {
      writeFileSync(join(dir, 'call.json'), JSON.stringify(document));
      return run(binary, ['call', 'holding.calculate.runa', '--entry', entry, '--input', 'call.json'], dir);
    },
    async callPartial() { throw new Error('not used'); },
  };
  const usage = await import(pathToFileURL(join(dir, 'build/usage.js')));
  assert.equal(await usage.result(cli), 1200);
});

test('partial values, dates, paths and explained calls are checked by the types', options, async () => {
  const dir = project('ledger');
  writeFileSync(join(dir, 'ledger.calculate.runa'), ledgerModel);
  run(binary, ['sdk', 'ts', 'ledger.calculate.runa', '--out', 'sdk'], dir);
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
@ calculate
> balance(input: Ledger) -> Int { foldl(input.events, 0, |sum, e| sum + signed(e)) }
`;

const keyedUsage = `import {
  answer, ask, balanceQuery, resolve,
  type BalanceElementKey, type BalanceNeed, type BalanceResolvers, type EngineTransport, type PartialEvent, type PartialLedger,
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
    events: async () => {
      const skeletons = await backend.skeletons();
      return () => answer(skeletons);
    },
    "events[].amount": async (needs) => {
      const rows = await backend.details(needs.map(eventId));
      return (need) => {
        const row = rows.get(eventId(need));
        return row === undefined ? ask : answer(row.amount);
      };
    },
    "events[].fee": async (needs) => {
      const rows = await backend.details(needs.map(eventId));
      return (need) => {
        const row = rows.get(eventId(need));
        return row === undefined ? ask : answer(row.fee);
      };
    },
  };
}

export const interview = (transport: EngineTransport, backend: Backend) =>
  resolve((c: PartialLedger) => balanceQuery(transport, c), {}, resolvers(backend));
`;

test('second-order resolution: a list of skeletons, then the fields the law reads, by key', options, async () => {
  const dir = project('keyed');
  writeFileSync(join(dir, 'keyed.calculate.runa'), keyedModel);
  run(binary, ['sdk', 'ts', 'keyed.calculate.runa', '--out', 'sdk'], dir);
  writeFileSync(join(dir, 'usage.ts'), keyedUsage);
  compile(dir, ['sdk/index.ts', 'usage.ts']);
  const sdk = await import(pathToFileURL(join(dir, 'build/sdk/index.js')));
  const usage = await import(pathToFileURL(join(dir, 'build/usage.js')));
  assert.match(readFileSync(join(dir, 'sdk/partial.ts'), 'utf8'),
    /export type BalanceElementKey =\n {2}\| ElementKey<"events\[\]", "id", string>;/);

  const queried = [];
  const transport = {
    async call() { throw new Error('not used'); },
    async callPartial(entry, document) {
      const path = join(dir, `partial-${queried.length}.json`);
      queried.push(document.cases[0].input);
      writeFileSync(path, JSON.stringify(document));
      return run(binary, ['call', 'keyed.calculate.runa', '--entry', entry, '--input', path, '--partial'], dir);
    },
  };
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

  // Round 1 asks for the list, round 2 for each event's fields by its id: one backend call per pattern per round.
  queried.length = 0;
  const rows = new Map([['e1', { amount: 100, fee: 0 }], ['e2', { amount: 30, fee: 2 }], ['e3', { amount: 7, fee: 0 }]]);
  const backend = { skeletons: 0, details: [] };
  const resolution = await usage.interview(transport, {
    async skeletons() {
      backend.skeletons += 1;
      return skeletons;
    },
    async details(ids) {
      backend.details.push([...ids]);
      return new Map(ids.map((id) => [id, rows.get(id)]));
    },
  });
  assert.equal(resolution.$variant, 'Complete', JSON.stringify(resolution));
  assert.equal(resolution.result, 100 - 30 - 2 + 7);
  assert.equal(resolution.rounds, 3);
  assert.deepEqual(queried, [{}, { events: skeletons }, { events: [
    { $variant: 'Deposit', id: 'e1', amount: 100 }, { $variant: 'Withdrawal', id: 'e2', amount: 30, fee: 2 },
    { $variant: 'Deposit', id: 'e3', amount: 7 }] }]);
  assert.equal(backend.skeletons, 1);
  assert.deepEqual(backend.details.sort(), [['e1', 'e2', 'e3'], ['e2']]);
});

test('the generated SDK passes typescript-eslint strictTypeChecked', {
  skip: !(ready && existsSync(join(deps, 'node_modules/typescript-eslint')))
    && 'Set FUTURUNA_SDK_TS_DEPS to node_modules that also hold eslint and typescript-eslint.',
}, () => {
  const dir = project('lint');
  run(binary, ['sdk', 'ts', fixture, '--out', join(dir, 'sdk/tax'), '--runtime', '../runtime.ts']);
  writeFileSync(join(dir, 'ledger.calculate.runa'), ledgerModel);
  run(binary, ['sdk', 'ts', 'ledger.calculate.runa', '--out', 'sdk/ledger', '--runtime', '../runtime.ts'], dir);
  writeFileSync(join(dir, 'tsconfig.json'), JSON.stringify({
    compilerOptions: { target: 'es2023', module: 'nodenext', moduleResolution: 'nodenext', strict: true,
      noUncheckedIndexedAccess: true, exactOptionalPropertyTypes: true, noEmit: true, types: [] },
    include: ['sdk/**/*.ts'],
  }));
  writeFileSync(join(dir, 'eslint.config.mjs'), `import tseslint from "typescript-eslint";
export default tseslint.config(...tseslint.configs.strictTypeChecked, {
  languageOptions: { parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname } },
});
`);
  run(process.execPath, [join(deps, 'node_modules/eslint/bin/eslint.js'), '--max-warnings', '0', 'sdk'], dir);
});
