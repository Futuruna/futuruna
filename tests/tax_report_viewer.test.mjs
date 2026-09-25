// Synthetic output only. Fast presentation/CLI guards, not tax-law evidence.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { parseReportOutput, renderReportOutput } from '../examples/danish-income-tax/afstemning-resultat.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const script = join(root, 'examples/danish-income-tax/afstemning-resultat.mjs');
const variant = $variant => ({ $variant });
function baseline() {
  return {
    $futuruna: { schema: 'futuruna.calculate.output.v1', schema_hash: 'a'.repeat(64), entry: 'afstem_årsopgørelse' },
    results: [{ case_id: 'fiktiv-rapport', result: {
      status: variant('BetingetAfstemt'), uafhængig_skatteberegning_udført: false,
      kontroller: [{ navn: 'Fiktiv kontrol', enhed: 'øre', forventet: 380000, oplyst: 380000, difference: 0, status: variant('Stemmer') }],
      nødvendige_forudsætninger: [{ navn: 'Nødvendigt fiktivt beløb', enhed: 'DKK', nødvendigt_beløb: 12000,
        mindst: 0, højst: null, inden_for_kontrollerede_grænser: true, forklaring: 'Ikke dokumentation for en ægtefælles forhold.' }],
      uafklaret: ['Kildeoplysninger er ikke kontrolleret.', 'Den anden betingelse er også uafklaret.'],
    } }], diagnostics: [],
  };
}
const render = value => renderReportOutput(parseReportOutput(JSON.stringify(value)));
const resultOf = value => value.results[0].result;

test('Danish report preserves every check, condition, caveat and exact unit', () => {
  const source = baseline();
  const before = JSON.stringify(source);
  const { text, exitCode } = render(source);
  assert.equal(exitCode, 0);
  assert.match(text, /Betinget afstemt/);
  assert.match(text, /ikke en uafhængig skatteberegning/);
  assert.match(text, /ikke kontrolleret mod den aktuelle model/);
  assert.match(text, /3\.800,00 kr\. \(380\.000 øre\)/);
  assert.match(text, /difference: 0,00 kr\. \(0 øre\)/);
  assert.match(text, /12\.000 DKK/);
  assert.match(text, /højst: ukendt — ikke ubegrænset ret/);
  for (const row of resultOf(source).kontroller) assert.ok(text.includes(row.navn));
  for (const row of resultOf(source).nødvendige_forudsætninger) {
    assert.ok(text.includes(row.navn)); assert.ok(text.includes(row.forklaring));
  }
  for (const caveat of resultOf(source).uafklaret) assert.ok(text.includes(caveat));
  assert.equal(JSON.stringify(source), before);
});

test('missing observations retain available necessary amounts, never fake zeros', () => {
  const source = baseline();
  const result = resultOf(source);
  result.status = variant('Ufuldstændig');
  Object.assign(result.kontroller[0], { oplyst: null, difference: null, status: variant('IkkeOplyst') });
  const output = render(source);
  assert.equal(output.exitCode, 2);
  assert.match(output.text, /oplyst: ukendt; difference: ukendt/);
  assert.match(output.text, /12\.000 DKK/);
  assert.match(output.text, /Ikke oplyst/);
});

test('one-ore contradictions and known zero ceilings remain visible', () => {
  const source = baseline();
  const result = resultOf(source);
  result.status = variant('Modstrid');
  Object.assign(result.kontroller[0], { oplyst: 379999, difference: -1, status: variant('Afviger') });
  Object.assign(result.nødvendige_forudsætninger[0], {
    enhed: 'øre', nødvendigt_beløb: -1, højst: 0, inden_for_kontrollerede_grænser: false,
  });
  const output = render(source);
  assert.equal(output.exitCode, 2);
  assert.match(output.text, /difference: -0,01 kr\. \(-1 øre\)/);
  assert.match(output.text, /højst: 0,00 kr\. \(0 øre\)/);
  assert.match(output.text, /grænser: NEJ/);
});

test('invalid and unsupported statuses never become an empty success', () => {
  for (const status of ['UgyldigtRapportinput', 'IkkeUnderstøttetÅrEllerKommune']) {
    const source = baseline();
    Object.assign(resultOf(source), { status: variant(status), kontroller: [], nødvendige_forudsætninger: [] });
    const output = render(source);
    assert.equal(output.exitCode, 2);
    assert.match(output.text, /ingen afstemning udført/);
    assert.match(output.text, /Ingen nødvendige beløb returneret; det betyder ikke nul/);
  }
});

test('mixed and diagnostic-only batches retain all case errors and locations', () => {
  const source = baseline();
  source.diagnostics.push({ case_id: 'afvist', path: '$.skat', message: 'Fiktiv typefejl' },
    { case_id: 'afvist', path: '$.betaling', message: 'En anden fiktiv fejl' });
  for (const results of [source.results, []]) {
    const output = render({ ...source, results });
    assert.equal(output.exitCode, 2);
    assert.match(output.text, /SAGER UDEN BEREGNINGSRESULTAT/);
    if (results.length > 0) assert.ok(output.text.indexOf('Kræver gennemgang:') < output.text.indexOf('\nSag:'));
    for (const error of source.diagnostics) {
      assert.ok(output.text.includes(error.case_id));
      assert.ok(output.text.includes(error.path)); assert.ok(output.text.includes(error.message));
    }
  }
});

test('integer parsing and display retain i64 extrema above Number precision', () => {
  const parsed = parseReportOutput('[-9223372036854775808,9223372036854775807,9007199254740993]');
  assert.deepEqual(parsed, [-9223372036854775808n, 9223372036854775807n, 9007199254740993n]);
  const source = parseReportOutput(JSON.stringify(baseline()));
  Object.assign(resultOf(source).kontroller[0], { forventet: 9223372036854775807n, oplyst: 9223372036854775807n });
  Object.assign(resultOf(source).nødvendige_forudsætninger[0], { nødvendigt_beløb: 9007199254740993n });
  const output = renderReportOutput(source);
  assert.match(output.text, /92\.233\.720\.368\.547\.758,07 kr\. \(9\.223\.372\.036\.854\.775\.807 øre\)/);
  assert.match(output.text, /9\.007\.199\.254\.740\.993 DKK/);
});

test('strict parser rejects ambiguous, lossy, malformed and excessive-depth output', () => {
  for (const text of [
    '{"x":1,"x":2}', '{"x":1,"\\u0078":2}', '{"nested":{"a":1,"a":2}}',
    '1.0', '1e3', '01', 'NaN', '9223372036854775808', '-9223372036854775809',
    '{"x":1,}', '[1,]', 'true false', '{"x" 1}', '"raw\nnewline"',
    '"\\q"', '{', '', '['.repeat(66) + '0' + ']'.repeat(66),
  ]) assert.throws(() => parseReportOutput(text), Error, text);
  assert.throws(() => parseReportOutput(' '.repeat(16 * 1024 * 1024 + 1)), /16 MiB/);
  const data = parseReportOutput(' { "tekst": "Ægtefælle \\"x\\" \\u00f8", "flags": [true, false, null], "__proto__": {} } ');
  assert.equal(data.tekst, 'Ægtefælle "x" ø');
  assert.deepEqual(data.flags, [true, false, null]);
  assert.equal(Object.getPrototypeOf(data), null);
  assert.ok(Object.hasOwn(data, '__proto__'));
});

test('wrong contracts and incompatible or contradictory presentation structures fail closed', () => {
  const edits = [
    value => { value.$futuruna.entry = 'beregn_personskat'; },
    value => { value.$futuruna.schema = 'futuruna.calculate.input.v1'; },
    value => { value.$futuruna.schema_hash = 'bad'; },
    value => { resultOf(value).uafhængig_skatteberegning_udført = true; },
    value => { resultOf(value).status = variant('NyUkendtStatus'); },
    value => { resultOf(value).uafklaret = []; },
    value => { resultOf(value).kontroller = []; },
    value => { resultOf(value).kontroller[0].difference = 1; },
    value => { resultOf(value).kontroller[0].enhed = 'EUR'; },
    value => { resultOf(value).nødvendige_forudsætninger[0].inden_for_kontrollerede_grænser = false; },
    value => { delete resultOf(value).kontroller[0].oplyst; },
    value => { resultOf(value).new_legal_warning = 'must not be hidden'; },
    value => { value.results.push(structuredClone(value.results[0])); },
    value => { value.diagnostics.push({ case_id: 'fiktiv-rapport', path: '$', message: 'Failed' }); },
    value => { value.results = []; },
  ];
  for (const edit of edits) { const source = baseline(); edit(source); assert.throws(() => render(source)); }
});

test('terminal controls in source text cannot rewrite displayed conclusions', () => {
  const source = baseline();
  source.results[0].case_id = 'navn\nFAKE\u001b[2J\u202e';
  resultOf(source).uafklaret.push('forbehold\rFAKE\u009b2J');
  const output = render(source);
  assert.ok(output.text.includes('navn\\u000aFAKE\\u001b[2J\\u202e'));
  assert.ok(output.text.includes('forbehold\\u000dFAKE\\u009b2J'));
  assert.doesNotMatch(output.text, /[\r\u001b\u009b\u202e]/);
});

test('CLI is read-only, emits no partial success on bad JSON, and uses distinct exit codes', () => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-report-viewer-'));
  const path = join(directory, 'synthetic.json');
  const invoke = args => spawnSync(process.execPath, [script, ...args], { cwd: root, encoding: 'utf8' });
  writeFileSync(path, JSON.stringify(baseline()), { flag: 'wx', mode: 0o600 });
  const bytes = readFileSync(path);
  const entries = readdirSync(directory);
  const valid = invoke([path]);
  assert.equal(valid.status, 0, valid.stderr);
  assert.match(valid.stdout, /Betinget afstemt/);
  assert.deepEqual(readFileSync(path), bytes);
  assert.deepEqual(readdirSync(directory), entries);
  const invalidPath = join(directory, 'ambiguous.json');
  writeFileSync(invalidPath, '{"private": "do-not-echo", "private":0}', { flag: 'wx' });
  const invalid = invoke([invalidPath]);
  assert.equal(invalid.status, 1);
  assert.equal(invalid.stdout, '');
  assert.doesNotMatch(invalid.stderr, /do-not-echo/);
  assert.match(invalid.stderr, /Gentaget JSON-feltnavn/);
  assert.equal(invoke([join(directory, 'missing.json')]).status, 1);
  assert.equal(invoke([directory]).status, 1);
  assert.equal(invoke([]).status, 1);
  assert.equal(invoke(['--help']).status, 0);
  const invalidEncodingPath = join(directory, 'invalid-utf8.json');
  writeFileSync(invalidEncodingPath, Buffer.from([0xff]), { flag: 'wx' });
  const invalidEncoding = invoke([invalidEncodingPath]);
  assert.equal(invalidEncoding.status, 1);
  assert.equal(invalidEncoding.stdout, '');
  const incomplete = baseline();
  resultOf(incomplete).status = variant('Ufuldstændig');
  Object.assign(resultOf(incomplete).kontroller[0], { forventet: null, difference: null, status: variant('IkkeOplyst') });
  const incompletePath = join(directory, 'incomplete.json');
  writeFileSync(incompletePath, JSON.stringify(incomplete), { flag: 'wx' });
  const pending = invoke([incompletePath]);
  assert.equal(pending.status, 2, pending.stderr);
  assert.match(pending.stdout, /Ufuldstændig/);
});

test('fresh compact-model output stays readable without dropping any returned evidence', {
  skip: !process.env.FUTURUNA_MODEL_TEST_RUNA && 'Set FUTURUNA_MODEL_TEST_RUNA to the verified compiler; no build is started.',
}, () => {
  const demo = spawnSync(process.execPath,
    ['examples/danish-income-tax/afstemning-demo.mjs', process.env.FUTURUNA_MODEL_TEST_RUNA],
    { cwd: root, encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 });
  assert.equal(demo.status, 0, demo.stderr);
  const directory = demo.stderr.match(/Input og resultater gemmes i: (.+)/)?.[1];
  assert.ok(directory, demo.stderr);
  const file = join(directory, 'results.json');
  const sourceBytes = readFileSync(file);
  const output = JSON.parse(sourceBytes);
  const view = spawnSync(process.execPath, [script, file], { cwd: root, encoding: 'utf8' });
  assert.equal(view.status, 2, view.stderr);
  const sections = view.stdout.split('\nSag: ').slice(1);
  assert.equal(sections.length, 4);
  for (const [index, row] of output.results.entries()) {
    const shown = sections[index];
    assert.ok(shown.startsWith(row.case_id));
    for (const check of row.result.kontroller) assert.ok(shown.includes(check.navn), check.navn);
    for (const condition of row.result.nødvendige_forudsætninger) {
      assert.ok(shown.includes(condition.navn), condition.navn);
      assert.ok(shown.includes(condition.forklaring), condition.navn);
    }
    for (const caveat of row.result.uafklaret) assert.ok(shown.includes(caveat), caveat);
  }
  assert.match(sections[1], /oplyst: ukendt; difference: ukendt/);
  assert.match(sections[2], /difference: 0,01 kr\. \(1 øre\)/);
  assert.match(sections[3], /ikke-oplyste ægtefællenedslag: -0,01 kr\. \(-1 øre\)/);
  assert.deepEqual(readFileSync(file), sourceBytes);
});
