// Fictional annual totals, not payroll correction deltas or an external oracle.
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
const model = 'examples/danish-income-tax/personskat.calculate.runa';
const enabled = { skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no build or network.' };
const v = ($variant, fields = {}) => ({ $variant, ...fields });
function run(args) {
  // Cold validation can exceed ten minutes on the shared 8 GB development host.
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 1200000,
    maxBuffer: 64 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
}
function save(dir, name, value) {
  const path = join(dir, name);
  writeFileSync(path, JSON.stringify(value), { flag: 'wx', mode: 0o600 }); return path;
}
function baseline(envelope) {
  const input = buildFictionalCases(envelope).envelope.cases.find(c => c.case_id === 'privat-rate').input;
  input.lønmodtager.pension.pbl18_indbetalinger = []; return input;
}
function spouseFacts(person) {
  return Object.fromEntries(['lønmodtager', 'kapitalindkomst', 'aktieavance',
    'udenlandske_sociale_bidrag', 'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter']
    .map(key => [key, structuredClone(person[key])]));
}
function su(person) {
  person.lønmodtager.personlig_indkomst.ordinære_forhold.forenings_og_arbejdsløshedsydelser = [
    v('PersonskatUddannelsesstøtte', { fakta: { identifikation: 'fictional-su', indkomstår: 2025,
      kildereference: 'fictional-su:1', art: v('SuStipendiumEfterDanskSuLov'),
      beløb_før_skat_kroner: 100000, uden_udlandsforhold_og_korrektioner: true } }),
  ];
}

test('negative ordinary wage totals cannot masquerade as supported annual corrections', enabled, async t => {
  const dir = mkdtempSync(join(tmpdir(), 'futuruna-salary-input-'));
  console.log(`Fictional salary evidence: ${dir}`);
  const envelope = run(['template', model, '--format', 'json']), base = baseline(envelope);
  const cases = [];
  function add(case_id, wage, edit = () => {}, spouse = false) {
    const input = structuredClone(base), person = spouse ? spouseFacts(base) : input;
    person.lønmodtager.bruttoløn_kroner = wage; edit(person);
    if (spouse) input.ægtefælle = v('MedÆgtefælle', { fakta: person,
      samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] });
    cases.push({ case_id, input, wage, spouse });
  }
  add('zero', 0);
  add('one-krone', 1);
  add('negative-one-krone', -1);
  add('ordinary-wage', 600000);
  // Fiction: corrected annual total is documented as 550000, not a -50000 delta.
  // This control does not determine entitlement to the source correction.
  add('documented-corrected-total', 550000);
  add('su-control', 0, su);
  add('negative-wage-offsetting-su', -10000, su);
  add('negative-spouse-wage', -10000, su, true);
  add('lawful-negative-capital', 0, p => { p.kapitalindkomst.renter.renteudgifter_kroner = 10000; });
  envelope.cases = cases.map(({ case_id, input }) => ({ case_id, input }));
  const output = run(['call', model, '--input', save(dir, 'input.json', envelope)]);
  save(dir, 'results.json', output);
  assert.deepEqual(output.diagnostics, []);
  assert.deepEqual(output.results.map(r => r.case_id), cases.map(c => c.case_id));
  for (const [i, row] of output.results.entries()) {
    const c = cases[i], r = row.result, a = r.vurdering;
    console.log(`${c.case_id}: valid=${a.alle_kontroller_gyldige}; tax-øre=${a.slutskat_til_sammenligning_øre}`);
    await t.test(c.case_id, () => {
      const valid = c.wage >= 0;
      assert.equal(a.alle_kontroller_gyldige, valid);
      assert.equal(a.status.$variant, valid ? 'BeregnetMedForbehold' : 'UgyldigtBeregningsgrundlag');
      const path = `${c.spouse ? 'ægtefælle.MedÆgtefælle.fakta.' : ''}lønmodtager.bruttoløn_kroner`;
      const controls = a.kontroller.filter(f => f.sti === path);
      assert.equal(controls.length, 1);
      assert.equal(controls[0].gyldig, valid);
      // Invalid values remain diagnostic evidence, not clipped to zero/abs.
      assert.equal(c.spouse ? r.ægtefælle.fakta.lønmodtager.bruttoløn_kroner : r.skat.bruttoløn_kroner, c.wage);
      if (valid) assert.equal(typeof a.slutskat_til_sammenligning_øre, 'number');
      else {
        assert.equal(a.slutskat_til_sammenligning_øre, null);
        assert.deepEqual(a.fejl.map(f => f.sti), [path], 'isolated wage coverage failure');
        assert.ok(controls[0].forklaring.includes('ikke en afgørelse') || controls[0].forklaring.includes('afgør ikke'));
      }
    });
  }
  const results = new Map(output.results.map(r => [r.case_id, r.result]));
  assert.equal(results.get('ordinary-wage').vurdering.slutskat_til_sammenligning_øre, 21194454);
  assert.equal(results.get('su-control').vurdering.slutskat_til_sammenligning_øre, 1718684);
  for (const id of ['zero', 'one-krone', 'lawful-negative-capital'])
    assert.equal(results.get(id).vurdering.slutskat_til_sammenligning_øre, 0, id);
  assert.equal(results.get('lawful-negative-capital').skat.nettokapitalindkomst_kroner, -10000);
  assert.equal(results.get('negative-wage-offsetting-su').slutskat_øre, 1311992,
    'keep the reproduced raw arithmetic diagnostic, but do not publish it as valid tax');
});

test('part-year comparison retains the wage boundary in period and documented annual bases', enabled, () => {
  const partModel = 'examples/danish-income-tax/personskat-par14.calculate.runa';
  const dir = mkdtempSync(join(tmpdir(), 'futuruna-salary-partyear-'));
  console.log(`Fictional part-year wage evidence: ${dir}`);
  const envelope = run(['template', partModel, '--format', 'json']);
  const base = baseline({ cases: [{ input: envelope.cases[0].input.personskat }] });
  function input(periodWage, annualWage) {
    const personskat = structuredClone(base), annual = structuredClone(base);
    personskat.lønmodtager.bruttoløn_kroner = periodWage;
    annual.lønmodtager.bruttoløn_kroner = annualWage;
    return { personskat, skattepligtsændring: v('FuldSkattepligtOphører'),
      skattepligtsperiode: { fra_dato: { år: 2025, måned: 1, dag: 1 }, til_dato: { år: 2025, måned: 6, dag: 30 } },
      valg_afgivet_ved_oplysninger: false, omvalg_dato: null,
      kilder: [{ identifikation: 'fictional-wage', beregningsfelt: v('Par14Bruttoløn'),
        delårsbeløb_kroner: periodWage,
        omregningsmetode: v('Par14DokumenteretRetvisendeHelårsbeløb', { helårsbeløb_kroner: annualWage }),
        faktisk_helårsbeløb_kroner: null }],
      helårsgrundlag: v('DokumenteretHelårsPersonskat', { personskat: annual }) };
  }
  const specs = [['control', 300000, 600000], ['negative-period', -1, 600000], ['negative-annual', 300000, -1],
    ['negative-period-spouse', 300000, 600000], ['future-birth-date', 300000, 600000]];
  envelope.cases = specs.map(([case_id, periodWage, annualWage]) => {
    const c = input(periodWage, annualWage);
    if (case_id === 'negative-period-spouse') {
      for (const p of [c.personskat, c.helårsgrundlag.personskat]) p.ægtefælle = v('MedÆgtefælle', {
        fakta: spouseFacts(p), samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [],
      });
      c.personskat.ægtefælle.fakta.lønmodtager.bruttoløn_kroner = -1;
    }
    if (case_id === 'future-birth-date') for (const p of [c.personskat, c.helårsgrundlag.personskat])
      p.lønmodtager.pension.fødselsdato.år = 2026;
    return { case_id, input: c };
  });
  const output = run(['call', partModel, '--input', save(dir, 'input.json', envelope)]);
  save(dir, 'results.json', output);
  assert.deepEqual(output.diagnostics, []);
  assert.deepEqual(output.results.map(r => r.case_id), specs.map(s => s[0]));
  for (const { case_id, result: r } of output.results) {
    const valid = case_id === 'control';
    console.log(`${case_id}: valid=${r.vurdering.alle_kontroller_gyldige}; tax-øre=${r.vurdering.slutskat_til_sammenligning_øre}`);
    assert.equal(r.vurdering.alle_kontroller_gyldige, valid, case_id);
    assert.equal(r.kanonisk_beregning_understøttet, valid, case_id);
    if (valid) assert.equal(r.vurdering.slutskat_til_sammenligning_øre, 10375224, 'preserve the recorded control');
    else {
      assert.equal(r.vurdering.slutskat_til_sammenligning_øre, null);
      assert.ok(r.vurdering.fejl.some(f => f.sti === 'personskat'));
    }
    assert.equal(r.delårsresultat.vurdering.alle_kontroller_gyldige, valid || case_id === 'negative-annual');
    if (case_id === 'negative-period') {
      assert.ok(r.delårsresultat.vurdering.fejl.some(f => f.sti === 'lønmodtager.bruttoløn_kroner'));
      assert.equal(r.delårsresultat.skat.bruttoløn_kroner, -1);
    }
    if (case_id === 'negative-annual') assert.equal(r.helårsinput.bruttoløn_kroner, -1);
    if (case_id === 'negative-period-spouse') assert.ok(r.delårsresultat.vurdering.fejl.some(f =>
      f.sti === 'ægtefælle.MedÆgtefælle.fakta.lønmodtager.bruttoløn_kroner'));
    if (case_id === 'future-birth-date') assert.ok(r.delårsresultat.vurdering.fejl.some(f =>
      f.sti === 'lønmodtager.pension.fødselsdato'));
  }
});
