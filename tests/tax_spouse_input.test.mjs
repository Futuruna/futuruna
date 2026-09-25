// Fictional intake only: unavailable spouse facts must not assert no spouse.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { buildFictionalCases } from '../examples/danish-income-tax/bilag-demo.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
const enabled = { skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build.' };
const model = 'examples/danish-income-tax/personskat.calculate.runa';
const unknown = { $variant: 'ÆgtefællegrundlagUoplyst' };
function run(args, status = 0) {
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 1200000,
    maxBuffer: 32 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, status, p.stderr);
  return JSON.parse(p.stdout);
}
function save(dir, name, value) {
  const path = join(dir, name);
  writeFileSync(path, JSON.stringify(value), { flag: 'wx', mode: 0o600 });
  return path;
}
function missingAssessment(result) {
  assert.equal(result.vurdering.alle_kontroller_gyldige, false);
  assert.equal(result.vurdering.slutskat_til_sammenligning_øre, null);
  const error = result.vurdering.fejl.find(f => f.sti === 'ægtefælle.$variant');
  assert.ok(error);
  assert.match(error.forklaring, /aarsopgoerelse-afstemning/);
}

test('fresh template does not assert an absent spouse', enabled, () => {
  const template = run(['template', model, '--format', 'json']);
  assert.deepEqual(template.cases[0].input.ægtefælle, unknown);
});

test('unknown spouse basis withholds comparison while explicit known cases remain usable', enabled, () => {
  const envelope = run(['template', model, '--format', 'json']);
  const base = buildFictionalCases(envelope).envelope.cases[0].input;
  const missing = structuredClone(base); missing.ægtefælle = unknown;
  const married = structuredClone(base);
  const fakta = Object.fromEntries(['lønmodtager', 'kapitalindkomst', 'aktieavance',
    'udenlandske_sociale_bidrag', 'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter']
    .map(key => [key, structuredClone(base[key])]));
  married.ægtefælle = { $variant: 'MedÆgtefælle', fakta,
    samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] };
  const incomplete = structuredClone(married);
  incomplete.ægtefælle.fakta.lønmodtager.pension.atp = { $variant: 'AtpUoplyst' };
  envelope.cases = [['known-no-spouse', base], ['unknown-basis', missing],
    ['known-spouse', married], ['incomplete-spouse', incomplete]]
    .map(([case_id, input]) => ({ case_id, input }));
  const evidence = mkdtempSync(join(tmpdir(), 'futuruna-spouse-intake-'));
  console.log(`Fictional spouse intake: ${evidence}`);
  const out = run(['call', model, '--input', save(evidence, 'input.json', envelope)]);
  save(evidence, 'results.json', out);
  assert.deepEqual(out.diagnostics, []);
  assert.deepEqual(out.results.map(r => r.case_id), envelope.cases.map(r => r.case_id));
  const [single, absent, couple, partial] = out.results.map(r => r.result);
  for (const known of [single, couple]) {
    assert.equal(known.vurdering.alle_kontroller_gyldige, true);
    assert.equal(known.vurdering.slutskat_til_sammenligning_øre, 19661194);
  }
  missingAssessment(absent);
  assert.equal(partial.vurdering.slutskat_til_sammenligning_øre, null);
  assert.ok(partial.vurdering.fejl.some(f => f.sti === 'ægtefælle.MedÆgtefælle.fakta.lønmodtager.pension.atp'));
  // Removing the required relationship entirely must not choose a default.
  const omitted = structuredClone(base); delete omitted.ægtefælle;
  envelope.cases = [{ case_id: 'omitted-spouse-field', input: omitted }];
  const rejected = run(['call', model, '--input', save(evidence, 'omitted.json', envelope)], 1);
  save(evidence, 'omitted-results.json', rejected);
  assert.deepEqual(rejected.results, []);
  assert.ok(rejected.diagnostics.some(d => d.case_id === 'omitted-spouse-field'));
});

test('spouse interview metadata follows annual, part-year and green-check inputs', enabled, () => {
  for (const [file, entry, paths] of [
    [model, 'beregn_personskat', ['ægtefælle.$variant']],
    ['examples/danish-income-tax/personskat-par14.calculate.runa', 'beregn_personskat_delår',
      ['personskat.ægtefælle.$variant', 'helårsgrundlag.DokumenteretHelårsPersonskat.personskat.ægtefælle.$variant']],
    ['examples/danish-income-tax/personskat-groen-check.calculate.runa', 'beregn_personskat_med_grøn_check',
      ['personskat.ægtefælle.$variant']],
  ]) {
    const schema = run(['schema', file, '--entry', entry, '--format', 'compact-json']);
    for (const path of paths) {
      const fields = schema.field_metadata.filter(f => f.path === path);
      assert.equal(fields.length, 1, path);
      const field = fields[0];
      assert.equal(field.anchor, 'PersonskatÆgtefælleInput');
      for (const text of ['ÆgtefællegrundlagUoplyst', 'UdenÆgtefælle', 'MedÆgtefælle',
        'nul', 'årsopgørelse', 'aarsopgoerelse-afstemning.md']) assert.ok(field.help.includes(text), `${path}: ${text}`);
      const sources = schema.source_groups[field.source_group].map(id => schema.source_objects[id]);
      assert.ok(sources.some(s => s.role === 'guidance'
        && JSON.stringify(s).includes('https://skat.dk/borger/forskudsopgoerelse/aegteskab-skilsmisse-og-skat')));
      assert.ok(sources.some(s => s.role === 'warning' && JSON.stringify(s).includes('ÆgtefællegrundlagUoplyst')));
    }
  }
});

test('unresolved spouse basis cannot bypass the period or documented annual assessment', enabled, () => {
  const file = 'examples/danish-income-tax/personskat-par14.calculate.runa', entry = 'beregn_personskat_delår';
  const envelope = run(['template', file, '--entry', entry, '--format', 'json']);
  assert.deepEqual(envelope.cases[0].input.personskat.ægtefælle, unknown);
  const person = buildFictionalCases({ cases: [{ input: envelope.cases[0].input.personskat }] }).envelope.cases[0].input;
  person.lønmodtager.bruttoløn_kroner = 300000;
  person.lønmodtager.pension.pbl18_indbetalinger = [];
  const annual = structuredClone(person); annual.lønmodtager.bruttoløn_kroner = 604972;
  const base = { personskat: person, skattepligtsændring: { $variant: 'FuldSkattepligtOphører' },
    skattepligtsperiode: { fra_dato: { år: 2025, måned: 1, dag: 1 }, til_dato: { år: 2025, måned: 6, dag: 30 } },
    valg_afgivet_ved_oplysninger: false, omvalg_dato: null,
    kilder: [{ identifikation: 'fiktiv-loen', beregningsfelt: { $variant: 'Par14Bruttoløn' }, delårsbeløb_kroner: 300000,
      omregningsmetode: { $variant: 'Par14ForholdsmæssigtLøbendeBeløb' }, faktisk_helårsbeløb_kroner: null }],
    helårsgrundlag: { $variant: 'DokumenteretHelårsPersonskat', personskat: annual } };
  envelope.cases = ['known', 'period-unknown', 'annual-unknown'].map(case_id => {
    const input = structuredClone(base);
    if (case_id === 'period-unknown') input.personskat.ægtefælle = unknown;
    if (case_id === 'annual-unknown') input.helårsgrundlag.personskat.ægtefælle = unknown;
    return { case_id, input };
  });
  const evidence = mkdtempSync(join(tmpdir(), 'futuruna-spouse-partyear-'));
  console.log(`Fictional spouse part-year: ${evidence}`);
  const out = run(['call', file, '--entry', entry, '--input', save(evidence, 'input.json', envelope)]);
  save(evidence, 'results.json', out);
  assert.deepEqual(out.diagnostics, []);
  assert.deepEqual(out.results.map(r => r.case_id), envelope.cases.map(r => r.case_id));
  for (const { case_id, result: r } of out.results) {
    const valid = case_id === 'known';
    assert.equal(r.vurdering.alle_kontroller_gyldige, valid, case_id);
    assert.equal(r.vurdering.slutskat_til_sammenligning_øre, valid ? 10383101 : null, case_id);
    assert.equal(r.helårsgrundlag_gyldigt, case_id !== 'annual-unknown', case_id);
    if (case_id === 'period-unknown') missingAssessment(r.delårsresultat);
    if (case_id === 'annual-unknown') assert.ok(r.vurdering.fejl.some(f => f.sti === 'helårsgrundlag'));
  }
});
