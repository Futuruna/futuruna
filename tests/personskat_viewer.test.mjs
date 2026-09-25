// Fictional presentation fixtures only. These tests do not establish tax-law conformance.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { parseReportOutput } from '../examples/danish-income-tax/resultat-visning.mjs';
import { incomeFields, renderPersonskatOutput, resultFields } from '../examples/danish-income-tax/personskat-resultat.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const model = 'examples/danish-income-tax/personskat.calculate.runa';
const script = join(root, 'examples/danish-income-tax/personskat-resultat.mjs');
const variant = ($variant, fields = {}) => ({ $variant, ...fields });
const resultOf = value => value.results[0].result;
const render = value => renderPersonskatOutput(parseReportOutput(JSON.stringify(value)));
const invoke = args => spawnSync(process.execPath, [script, ...args], { cwd: root, encoding: 'utf8' });

function baseline() {
  // Non-summary internals are placeholders: the viewer explicitly does not
  // type-check the entire tax graph. The integration test uses real model output.
  const result = Object.fromEntries(resultFields.map(field => [field, {}]));
  Object.assign(result, {
    slutskat_øre: 1234567,
    vurdering: {
      status: variant('BeregnetMedForbehold'), alle_kontroller_gyldige: true,
      slutskat_til_sammenligning_øre: 1234567, samlet_modeldækning_bekræftet: false,
      kontroller: [{ sti: 'lønmodtager.pension.atp', gyldig: true, forklaring: 'Fiktiv kildekontrol.' }],
      fejl: [], forbehold: ['Kildefakta er ikke bekræftet.', 'En anden begrænsning bevares.'],
    },
    skat: { ...Object.fromEntries(incomeFields.map(([field]) => [field, 0])), skatteår: 2026 },
    ægtefælle: variant('IngenÆgtefælleberegning'), årsopgørelse: variant('IngenÅrsopgørelse'),
  });
  result.skat.nettokapitalindkomst_kroner = -15000;
  return {
    $futuruna: { schema: 'futuruna.calculate.output.v1', schema_hash: 'b'.repeat(64), entry: 'beregn_personskat' },
    results: [{ case_id: 'fiktiv-beregning', result }], diagnostics: [],
  };
}
function invalidate(source) {
  const a = resultOf(source).vurdering;
  a.status = variant('UgyldigtBeregningsgrundlag'); a.alle_kontroller_gyldige = false;
  a.slutskat_til_sammenligning_øre = null;
  a.kontroller[0].gyldig = false; a.fejl = [structuredClone(a.kontroller[0])];
  return source;
}

test('supported outer record and selected fields stay aligned with the source model', () => {
  const source = readFileSync(join(root, model), 'utf8');
  const fields = source.match(/^# PersonskatBeregningResultat\((.+)\)$/m)?.[1];
  assert.ok(fields, 'Review the viewer when the result declaration changes.');
  assert.deepEqual(fields.split(', ').map(f => f.split(':')[0]).sort(), [...resultFields].sort());
  const wages = readFileSync(join(root, 'examples/danish-income-tax/loenmodtager_beregning.runa'), 'utf8');
  const breakdown = wages.match(/^# LønmodtagerBreakdown\((.+)\)$/m)?.[1];
  for (const [field] of incomeFields) assert.ok(breakdown.includes(`${field}: Heltal`), field);
});

test('valid summary shows exact gated amounts, scope and every assessment control/caveat', () => {
  const source = baseline();
  resultOf(source).vurdering.kontroller.push({ sti: 'ægtefælle', gyldig: true, forklaring: 'Fiktiv inaktiv gren.' });
  const before = JSON.stringify(source);
  const { text, exitCode } = render(source);
  assert.equal(exitCode, 0);
  assert.match(text, /12\.345,67 kr\. \(1\.234\.567 øre\)/);
  assert.match(text, /Nettokapitalindkomst: -15\.000 DKK/);
  assert.match(text, /ikke restskat, overskydende skat eller en udbetaling/);
  assert.match(text, /Ingen sammenligning med et observeret beløb/);
  assert.match(text, /ikke kontrolleret mod den aktuelle model/);
  assert.match(text, /ikke en godkendt årsopgørelse/);
  assert.match(text, /Det fastslår ikke civilstand/);
  assert.match(text, /Det betyder ikke nul i restskat/);
  assert.match(text, /delbeløb kan overlappe/);
  assert.match(text, /faste modeltekster; de beskriver også fejlscenarier/);
  for (const c of resultOf(source).vurdering.kontroller) {
    assert.ok(text.includes(c.sti)); assert.ok(text.includes(c.forklaring));
  }
  for (const c of resultOf(source).vurdering.forbehold) assert.ok(text.includes(c));
  assert.equal(JSON.stringify(source), before);
});

test('invalid assessment withholds every diagnostic amount, including seemingly plausible tax', () => {
  const source = invalidate(baseline());
  const output = render(source);
  assert.equal(output.exitCode, 2);
  assert.match(output.text, /Beløb tilbageholdt — ukendt er ikke nul/);
  assert.match(output.text, /FEJL — lønmodtager.pension.atp: Fiktiv kildekontrol/);
  assert.doesNotMatch(output.text, /12\.345|1\.234\.567|-15\.000|Modelleret slutskat|Bruttoløn:/);
  // Empty controls are invalid, not vacuous success, matching the model gate.
  Object.assign(resultOf(source).vurdering, { kontroller: [], fejl: [] });
  assert.match(render(source).text, /Ingen kontroller returneret/);
});

test('an exact zero is distinct from unavailable tax and i64 values never lose an ore', () => {
  const source = parseReportOutput(JSON.stringify(baseline()));
  for (const [value, expected] of [[0n, '0,00 kr. (0 øre)'], [-1n, '-0,01 kr. (-1 øre)'],
    [9223372036854775807n, '92.233.720.368.547.758,07 kr. (9.223.372.036.854.775.807 øre)']]) {
    resultOf(source).slutskat_øre = value;
    resultOf(source).vurdering.slutskat_til_sammenligning_øre = value;
    const output = renderPersonskatOutput(source);
    assert.equal(output.exitCode, 0); assert.ok(output.text.includes(expected));
  }
});

test('mixed batches and diagnostics-only batches preserve each error and lead with attention', () => {
  const source = baseline();
  const failed = invalidate(baseline()).results[0]; failed.case_id = 'uoplyst-atp';
  source.results.push(failed);
  source.diagnostics.push({ case_id: 'ikke-understøttet', path: '$.input', message: 'Fiktivt år uden model.' },
    { case_id: 'ikke-understøttet', path: '$.andet', message: 'En anden diagnose.' });
  for (const results of [source.results, []]) {
    const output = render({ ...source, results });
    assert.equal(output.exitCode, 2);
    if (results.length) assert.ok(output.text.indexOf('Kræver gennemgang:') < output.text.indexOf('\nSag:'));
    for (const d of source.diagnostics) {
      for (const field of ['case_id', 'path', 'message']) assert.ok(output.text.includes(d[field]));
    }
  }
});

test('spouse and settlement presence never silently become a household or refund summary', () => {
  const source = baseline();
  resultOf(source).ægtefælle = variant('BeregnetÆgtefælle', { fakta: {}, grundlag: {}, skat: {}, samlevende_ved_indkomstårets_udløb: true });
  resultOf(source).årsopgørelse = variant('BeregnetÅrsopgørelse', { input: {}, resultat: {}, afregning: {} });
  const output = render(source);
  assert.match(output.text, /Kun hovedpersonens skat, ikke summen af begge ægtefællers skat/);
  assert.match(output.text, /ægtefællens egne tal vises ikke/);
  assert.match(output.text, /ikke vist eller valideret i denne oversigt/);
});

test('unknown shapes, missing warnings and contradictory gates are rejected', () => {
  const edits = [
    s => { s.$futuruna.entry = 'beregn_personskat_med_grøn_check'; },
    s => { s.$futuruna.schema = 'futuruna.calculate.input.v1'; },
    s => { s.$futuruna.schema_hash = 'not-a-hash'; },
    s => { resultOf(s).new_warning = 'Must not disappear'; },
    s => { delete resultOf(s).vurdering; },
    s => { resultOf(s).vurdering.status = variant('NyStatus'); },
    s => { resultOf(s).vurdering.additional_warning = 'Must not disappear'; },
    s => { resultOf(s).vurdering.forbehold = []; },
    s => { resultOf(s).vurdering.forbehold.push(' '); },
    s => { resultOf(s).vurdering.samlet_modeldækning_bekræftet = true; },
    s => { resultOf(s).vurdering.slutskat_til_sammenligning_øre = null; },
    s => { resultOf(s).vurdering.slutskat_til_sammenligning_øre += 1; },
    s => { resultOf(s).vurdering.alle_kontroller_gyldige = false; },
    s => { resultOf(s).vurdering.kontroller = []; },
    s => { resultOf(s).vurdering.kontroller[0].gyldig = false; },
    s => { resultOf(s).vurdering.kontroller[0].gyldig = 'true'; },
    s => { resultOf(s).vurdering.kontroller[0].additional_warning = 'Must not disappear'; },
    s => { resultOf(s).vurdering.fejl = [structuredClone(resultOf(s).vurdering.kontroller[0])]; },
    s => { invalidate(s); resultOf(s).vurdering.fejl[0].forklaring = 'Changed'; },
    s => { invalidate(s); resultOf(s).vurdering.slutskat_til_sammenligning_øre = 0; },
    s => { resultOf(s).skat.bruttoløn_kroner = 1.5; },
    s => { resultOf(s).skat.bruttoløn_kroner = '10'; },
    s => { resultOf(s).årsopgørelse = variant('NyÅrsopgørelse'); },
    s => { resultOf(s).ægtefælle = variant('NyÆgtefælle'); },
    s => { s.results.push(structuredClone(s.results[0])); },
    s => { s.diagnostics.push({ case_id: s.results[0].case_id, path: '$', message: 'Contradiction' }); },
    s => { s.results = []; },
  ];
  for (const edit of edits) { const source = baseline(); edit(source); assert.throws(() => render(source), edit.toString()); }
});

test('terminal controls in identifiers, paths, diagnostics and caveats remain visible text', () => {
  const source = invalidate(baseline());
  source.results[0].case_id = 'navn\nFAKE\u001b[2J';
  resultOf(source).vurdering.forbehold.push('forbehold\rFAKE\u202e');
  resultOf(source).vurdering.kontroller[0].sti = 'sti\u009b2J';
  resultOf(source).vurdering.fejl[0].sti = 'sti\u009b2J';
  source.diagnostics.push({ case_id: 'afvist', path: '$.x\nFAKE', message: 'Fejl\u001b[2J' });
  const output = render(source);
  assert.ok(output.text.includes('navn\\u000aFAKE\\u001b[2J'));
  assert.ok(output.text.includes('sti\\u009b2J'));
  assert.ok(output.text.includes('forbehold\\u000dFAKE\\u202e'));
  assert.ok(output.text.includes('$.x\\u000aFAKE'));
  assert.doesNotMatch(output.text, /[\r\u001b\u009b\u202e]/);
});

test('CLI is read-only and validates the whole batch before printing any success', () => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-personskat-viewer-'));
  const save = (name, value) => {
    const path = join(directory, name);
    writeFileSync(path, value, { flag: 'wx', mode: 0o600 }); return path;
  };
  const path = save('valid.json', JSON.stringify(baseline()));
  const bytes = readFileSync(path); const entries = readdirSync(directory);
  const ok = invoke([path]);
  assert.equal(ok.status, 0, ok.stderr); assert.match(ok.stdout, /Beregnet med forbehold/);
  assert.deepEqual(readFileSync(path), bytes); assert.deepEqual(readdirSync(directory), entries);
  const bad = baseline();
  const later = structuredClone(bad.results[0]); later.case_id = 'later';
  later.result.vurdering.slutskat_til_sammenligning_øre += 1;
  bad.results.push(later);
  const refused = invoke([save('contradictory.json', JSON.stringify(bad))]);
  assert.equal(refused.status, 1); assert.equal(refused.stdout, '');
  for (const [name, text] of [['duplicate', '{"private":"do-not-echo","private":0}'],
    ['duplicate-escaped', '{"x":0,"\\u0078":1}'], ['fraction', '{"x":1.0}'],
    ['overflow', '[9223372036854775808]'], ['utf8', Buffer.from([0xff])]]) {
    const invalid = invoke([save(`${name}.json`, text)]);
    assert.equal(invalid.status, 1); assert.equal(invalid.stdout, '');
    assert.doesNotMatch(invalid.stderr, /do-not-echo/);
  }
  assert.equal(invoke([save('unknown.json', JSON.stringify(invalidate(baseline())))]).status, 2);
  assert.equal(invoke([join(directory, 'missing-private-name.json')]).status, 1);
  assert.doesNotMatch(invoke([join(directory, 'missing-private-name.json')]).stderr, /missing-private-name/);
  assert.equal(invoke([directory]).status, 1); assert.equal(invoke([]).status, 1);
  assert.equal(invoke(['--help']).status, 0);
});

test('fresh canonical output retains valid, unknown, spouse and failed-year cases', {
  skip: !process.env.FUTURUNA_MODEL_TEST_RUNA && 'Set FUTURUNA_MODEL_TEST_RUNA; no build or network is started.',
}, () => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-personskat-viewer-model-'));
  console.log(`Fictional canonical viewer evidence: ${directory}`);
  const save = (name, value) => {
    const path = join(directory, name);
    writeFileSync(path, JSON.stringify(value) + '\n', { flag: 'wx', mode: 0o600 }); return path;
  };
  const run = args => spawnSync(process.env.FUTURUNA_MODEL_TEST_RUNA, args, {
    cwd: root, encoding: 'utf8', timeout: 600000, maxBuffer: 16 * 1024 * 1024,
    env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' },
  });
  const template = run(['template', model, '--format', 'json']);
  assert.ifError(template.error); assert.equal(template.status, 0, template.stderr);
  const envelope = JSON.parse(template.stdout);
  const input = envelope.cases[0].input;
  // Explicit fictional adult, whole-year Denmark, no spouse, church, property,
  // capital income, losses, ATP/pension payments or other deductions. Empty
  // template branches describe this invented person ONLY, never missing data.
  Object.assign(input.lønmodtager, { skatteår: 2026, bruttoløn_kroner: 300000,
    kommune: variant('København'), betaler_kirkeskat: false });
  Object.assign(input.lønmodtager.pension, { fødselsdato: { år: 1990, måned: 1, dag: 1 },
    atp: variant('IngenAtpIndbetalinger'), udbetalingsoplysninger: { for_året_komplette: true, for_foregående_år_komplette: true } });
  Object.assign(input.lønmodtager.ligningsfradrag, {
    enlig_forsørger: variant('IntetEkstraBørnetilskud'), boligjob: variant('IngenBoligjobudgifter'),
    arbejdsfradrag_udland: variant('IngenUdlandsudelukkelseIFællesForhold', {
      dbo_hjemmehørende_udland_i_nogen_periode: false, noget_arbejde_udført_udland: null,
      nogen_udenlandsk_arbejdsgiver: null, kildereference: 'Fiktiv DBO-hjemmehørende DK hele året',
    }),
  });
  const missing = structuredClone(input); missing.lønmodtager.pension.atp = variant('AtpUoplyst');
  const future = structuredClone(input); future.lønmodtager.skatteår = 2030;
  const married = structuredClone(input);
  const spouseFields = ['lønmodtager', 'kapitalindkomst', 'aktieavance', 'udenlandske_sociale_bidrag',
    'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter'];
  const fakta = Object.fromEntries(spouseFields.map(f => [f, structuredClone(input[f])]));
  fakta.lønmodtager.bruttoløn_kroner = 0;
  married.ægtefælle = variant('MedÆgtefælle', { fakta, samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] });
  envelope.cases = [['beregnet', input], ['uoplyst-atp', missing], ['ægtefælle', married], ['ukendt-år', future]]
    .map(([case_id, input]) => ({ case_id, input }));
  const call = run(['call', model, '--input', save('cases.json', envelope)]);
  assert.ifError(call.error); assert.notEqual(call.status, 0, 'Unsupported year must produce a diagnostic.');
  const output = JSON.parse(call.stdout);
  const path = save('results.json', output);
  assert.deepEqual(output.results.map(r => r.case_id), ['beregnet', 'uoplyst-atp', 'ægtefælle']);
  assert.ok(output.diagnostics.length > 0);
  assert.ok(output.diagnostics.every(d => d.case_id === 'ukendt-år'));
  assert.deepEqual(output.results.map(r => r.result.vurdering.status.$variant),
    ['BeregnetMedForbehold', 'UgyldigtBeregningsgrundlag', 'BeregnetMedForbehold']);
  const bytes = readFileSync(path);
  const view = invoke([path]); assert.equal(view.status, 2, view.stderr);
  const sections = view.stdout.split('\nSag: ').slice(1);
  assert.equal(sections.length, 3);
  for (const [i, row] of output.results.entries()) {
    assert.ok(sections[i].startsWith(row.case_id));
    for (const c of row.result.vurdering.kontroller) {
      assert.ok(sections[i].includes(c.sti), c.sti); assert.ok(sections[i].includes(c.forklaring), c.forklaring);
    }
    for (const c of row.result.vurdering.forbehold) assert.ok(sections[i].includes(c));
  }
  assert.doesNotMatch(sections[1], /Modelleret slutskat|Bruttoløn:/);
  assert.match(sections[2], /ægtefællens egne tal vises ikke/);
  assert.match(view.stdout, /ukendt-år/);
  assert.deepEqual(readFileSync(path), bytes);
});
