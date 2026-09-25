// Source-backed input boundary, not an independent administrative tax oracle.
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
const model = 'examples/danish-income-tax/personskat-par14.calculate.runa';
const enabled = { skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no build or network.' };
const v = ($variant, fields = {}) => ({ $variant, ...fields });
function run(args) {
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 1200000,
    maxBuffer: 64 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr || p.stdout);
  return JSON.parse(p.stdout);
}
function save(directory, name, value) {
  const path = join(directory, name);
  writeFileSync(path, JSON.stringify(value), { flag: 'wx', mode: 0o600 }); return path;
}

test('part-year share sources cannot acquire salary-style annualization', enabled, async t => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-share-conversion-'));
  console.log(`Fictional share-conversion evidence: ${directory}`);
  const envelope = run(['template', model, '--format', 'json']);
  const person = buildFictionalCases({ cases: [{ input: envelope.cases[0].input.personskat }] })
    .envelope.cases[0].input;
  // Explicit fictional baseline, not the demo's document ledger: single,
  // Copenhagen2025, born1990, no church/ATP/pension/capital/other relief.
  // Liability July1-Dec31; wages200000, representative/actual annual400000.
  // One ordinary100000DKK dividend received within the Danish liability period.
  person.lønmodtager.pension.pbl18_indbetalinger = [];
  person.lønmodtager.bruttoløn_kroner = 200000;
  person.aktieavance.udbytter = [{ identifikation: 'in-period-dividend',
    udlodder: v('Ll16AAlmindeligtSelskab'), modtager: v('Ll16AAktuelAktionær'),
    aktiv: v('PersonskatAlmindeligAktie'), beløb_kroner: 100000,
    par13a_kildefakta: v('AblPar13AUdbytteUdenForModregningsgrundlag') }];
  const base = { personskat: person, skattepligtsændring: v('FuldSkattepligtIndtræder'),
    skattepligtsperiode: { fra_dato: { år: 2025, måned: 7, dag: 1 }, til_dato: { år: 2025, måned: 12, dag: 31 } },
    valg_afgivet_ved_oplysninger: false, omvalg_dato: null,
    kilder: [
      { identifikation: 'salary', beregningsfelt: v('Par14Bruttoløn'), delårsbeløb_kroner: 200000,
        omregningsmetode: v('Par14DokumenteretRetvisendeHelårsbeløb', { helårsbeløb_kroner: 400000 }),
        faktisk_helårsbeløb_kroner: 400000 },
      { identifikation: 'shares', beregningsfelt: v('Par14Aktieindkomst'), delårsbeløb_kroner: 100000,
        omregningsmetode: v('Par14UændretEngangsbeløb'), faktisk_helårsbeløb_kroner: 100000 },
    ], helårsgrundlag: v('AfledtFraIdentificeredeKilder') };
  const cases = [];
  function add(case_id, invalid, mutate = () => {}) {
    const input = structuredClone(base); mutate(input); cases.push({ case_id, input, invalid });
  }
  add('unchanged', false);
  add('proportional', true, x => { x.kilder[1].omregningsmetode = v('Par14ForholdsmæssigtLøbendeBeløb'); });
  add('documented-increase', true, x => {
    x.kilder[1].omregningsmetode = v('Par14DokumenteretRetvisendeHelårsbeløb', { helårsbeløb_kroner: 200000 });
  });
  add('actual-unchanged', false, x => { x.valg_afgivet_ved_oplysninger = true; });
  add('actual-ignores-unused-method', false, x => {
    x.valg_afgivet_ved_oplysninger = true;
    x.kilder[1].omregningsmetode = v('Par14ForholdsmæssigtLøbendeBeløb');
  });
  add('actual-increase', true, x => {
    x.valg_afgivet_ved_oplysninger = true; x.kilder[1].faktisk_helårsbeløb_kroner = 200000;
  });
  for (const amount of [100000, 200000]) add(`documented-basis-${amount}`, amount !== 100000, x => {
    const annual = structuredClone(person); annual.lønmodtager.bruttoløn_kroner = 400000;
    annual.aktieavance.udbytter[0].beløb_kroner = amount;
    x.helårsgrundlag = v('DokumenteretHelårsPersonskat', { personskat: annual });
    x.kilder[1].omregningsmetode = v('Par14DokumenteretRetvisendeHelårsbeløb', { helårsbeløb_kroner: amount });
  });
  add('offsetting-source-conversions', true, x => {
    const share = x.kilder.pop();
    for (const amount of [40000, 60000]) x.kilder.push({ ...structuredClone(share),
      identifikation: `shares-${amount}`, delårsbeløb_kroner: 50000, faktisk_helårsbeløb_kroner: 50000,
      omregningsmetode: v('Par14DokumenteretRetvisendeHelårsbeløb', { helårsbeløb_kroner: amount }) });
  });
  envelope.cases = cases.map(({ case_id, input }) => ({ case_id, input }));
  const output = run(['call', model, '--input', save(directory, 'input.json', envelope)]);
  save(directory, 'results.json', output);
  assert.deepEqual(output.diagnostics, []);
  assert.deepEqual(output.results.map(row => row.case_id), cases.map(row => row.case_id));
  for (const { case_id, result: r } of output.results) console.log(JSON.stringify({ case_id,
    valid: r.input_gyldigt, comparison: r.vurdering.slutskat_til_sammenligning_øre,
    faults: r.vurdering.fejl, shares: r.statslige_skattekomponenter.filter(c =>
      ['Par14EndeligAktieindkomstskat', 'Par14Par8AStk2Skat'].includes(c.art.$variant)) }));
  for (const [i, c] of cases.entries()) await t.test(c.case_id, () => {
    const r = output.results[i].result, a = r.vurdering;
    assert.equal(r.input_gyldigt, !c.invalid);
    assert.equal(a.alle_kontroller_gyldige, !c.invalid);
    assert.equal(a.samlet_modeldækning_bekræftet, false);
    if (c.invalid) {
      assert.equal(a.slutskat_til_sammenligning_øre, null);
      assert.ok(a.fejl.some(f => f.sti === 'kilder' && f.forklaring.includes('aktieindkomst')));
    } else {
      assert.deepEqual(a.fejl, []);
      assert.equal(a.slutskat_til_sammenligning_øre, 9821516);
      assert.equal(r.helårsskattekomponenter_afstemt, true);
      //2025single threshold67500: low18225 + high13650 =31875DKK.
      const low = r.statslige_skattekomponenter.find(c => c.art.$variant === 'Par14EndeligAktieindkomstskat');
      const high = r.statslige_skattekomponenter.find(c => c.art.$variant === 'Par14Par8AStk2Skat');
      assert.equal(low.delårsskat_øre, 1822500);
      assert.equal(high.delårsskat_øre, 1365000);
      assert.equal(low.helårsskat_øre, r.delårsresultat.endelig_aktieindkomstskat_øre);
    }
  });
});

test('share annualization warning reaches the generated source-choice fields', enabled, () => {
  const schema = run(['schema', model, '--format', 'compact-json']);
  for (const path of ['kilder.omregningsmetode.$variant', 'kilder.faktisk_helårsbeløb_kroner']) {
    const field = schema.field_metadata.find(f => f.path === path);
    assert.ok(field?.help.includes('aktieindkomst'), path);
    assert.ok(field.help.includes('uændret'), path);
    const sources = schema.source_groups[field.source_group].map(id => schema.source_objects[id]);
    assert.ok(sources.some(s => s.role === 'source' && JSON.stringify(s).includes('2021/1284')));
    assert.ok(sources.some(s => s.role === 'guidance' && JSON.stringify(s).includes('oid=1977389')));
  }
});
