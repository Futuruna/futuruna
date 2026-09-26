// Fictional source facts and recorded model controls, not an external tax oracle.
// Check the public contracts with one worker; never build a compiler here.
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
const enabled = { skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no build or network.' };
const canonical = 'examples/danish-income-tax/personskat.calculate.runa';
const partYear = 'examples/danish-income-tax/personskat-par14.calculate.runa';
const greenCheck = 'examples/danish-income-tax/personskat-groen-check.calculate.runa';
const v = ($variant, fields = {}) => ({ $variant, ...fields });
const creditNames = ['a_skat_og_am_indeholdt', 'par68_indbetalt', 'b_skat_betalt',
  'udbytteskat_modregningsberettiget', 'frivillig_indbetaling_par59', 'virksomhedsordning_beløb',
  'personskattelov_par8a_stk5_beløb', 'afskrivningslov_acontoskat', 'am_lov_par6_beløb',
  'seniornedslag', 'energiafgiftskompensation', 'tilbagebetalt_par55'];
function run(args) {
  // Cold validation on the shared 8 GB host may exceed ten minutes.
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 1200000,
    maxBuffer: 64 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
}
function person(input) {
  const p = buildFictionalCases({ cases: [{ input }] }).envelope.cases.find(c => c.case_id === 'privat-rate').input;
  p.lønmodtager.pension.pbl18_indbetalinger = []; return p;
}
const debt = (year = 2025) => v('AfregnRestskat', { fakta: { indkomstår: year,
  oplysningspligt_ikke_rettidigt_opfyldt: false, påbegyndte_måneder_fra_1_september: 0,
  øvrige_skyldige_renter_øre: 0 } });
const refund = (year = 2025) => v('AfregnOverskydendeSkat', { fakta: { indkomstår: year,
  modsvarer_par59_indbetaling_øre: 0, tilbagebetalt_rente_par59_øre: 0,
  modsvarer_seniornedslag_øre: 0, modsvarer_energiafgiftskompensation_øre: 0,
  restancer_personlig_skat_med_morarenter_øre: 0, tilbagebetaling_efter_1_september: false,
  påbegyndte_måneder_efter_1_september: 0 } });
function settlement(withheld, facts) {
  const kreditter = Object.fromEntries(creditNames.map(n => [n + '_øre', 0]));
  kreditter.a_skat_og_am_indeholdt_øre = withheld;
  return v('MedEksaktÅrsopgørelse', { overført_restskat_mv_øre: 0, øvrig_pensionsbeskatningsafgift_øre: 0,
    kreditter, afskrivningslov40c_acontoskat: v('UdenAfskrivningslov40CAcontoskat'), afregningsfakta: facts });
}
function call(model, envelope, cases) {
  const dir = mkdtempSync(join(tmpdir(), 'futuruna-settlement-stage-'));
  console.log(`Settlement-stage evidence (${model}): ${dir}`);
  envelope.cases = cases.map(({ case_id, input }) => ({ case_id, input }));
  writeFileSync(join(dir, 'input.json'), JSON.stringify(envelope), { flag: 'wx', mode: 0o600 });
  const out = run(['call', model, '--input', join(dir, 'input.json')]);
  writeFileSync(join(dir, 'results.json'), JSON.stringify(out), { flag: 'wx', mode: 0o600 });
  assert.deepEqual(out.diagnostics, []);
  assert.deepEqual(out.results.map(c => c.case_id), cases.map(c => c.case_id));
  return new Map(out.results.map(c => [c.case_id, c.result]));
}
function assessment(a, valid, calculationValid, settlementValid) {
  assert.equal(a.alle_kontroller_gyldige, valid);
  assert.equal(a.status.$variant, valid ? 'BeregnetMedForbehold' : 'UgyldigtBeregningsgrundlag');
  assert.equal(a.samlet_modeldækning_bekræftet, false);
  assert.equal(a.kontrolgrundlag.beregning.every(c => c.gyldig), calculationValid);
  assert.ok(a.kontrolgrundlag.beregning.length > 0);
  assert.equal(a.kontrolgrundlag.afregning.length, 1);
  assert.equal(a.kontrolgrundlag.afregning[0].gyldig, settlementValid);
  assert.deepEqual(a.kontroller, [...a.kontrolgrundlag.beregning, ...a.kontrolgrundlag.afregning]);
  assert.deepEqual(a.fejl, a.kontroller.filter(c => !c.gyldig));
  if (!valid) assert.equal(a.slutskat_til_sammenligning_øre, null);
}

test('annual assessment distinguishes source-credit checks from final payment checks', enabled, () => {
  const envelope = run(['template', canonical, '--format', 'json']);
  const base = person(envelope.cases[0].input);
  const cases = ['valid-debt', 'wrong-year', 'duplicate-source-credit'].map(case_id => {
    const input = structuredClone(base);
    input.årsopgørelse = settlement(0, debt(case_id === 'wrong-year' ? 2024 : 2025));
    if (case_id === 'duplicate-source-credit') input.årsopgørelse.kreditter.personskattelov_par8a_stk5_beløb_øre = 1;
    return { case_id, input };
  });
  const results = call(canonical, envelope, cases);
  for (const { case_id } of cases) {
    const r = results.get(case_id), valid = case_id === 'valid-debt';
    assessment(r.vurdering, valid, case_id !== 'duplicate-source-credit', case_id !== 'wrong-year');
    assert.equal(r.slutskat_øre, 21194454, 'settlement checks cannot change tax arithmetic');
    assert.equal(r.vurdering.kontrolgrundlag.beregning.filter(c => c.sti === 'årsopgørelse').length, 1);
    assert.equal(r.vurdering.kontrolgrundlag.afregning[0].sti, 'årsopgørelse');
    if (valid) assert.equal(r.vurdering.slutskat_til_sammenligning_øre, 21194454);
  }
});

test('part-year settlement follows the final tax without losing either source basis', enabled, () => {
  const envelope = run(['template', partYear, '--format', 'json']);
  const period = person(envelope.cases[0].input.personskat), annual = structuredClone(period);
  period.lønmodtager.bruttoløn_kroner = 300000;
  const base = { personskat: period, skattepligtsændring: v('FuldSkattepligtOphører'),
    skattepligtsperiode: { fra_dato: { år: 2025, måned: 1, dag: 1 }, til_dato: { år: 2025, måned: 6, dag: 30 } },
    valg_afgivet_ved_oplysninger: false, omvalg_dato: null,
    kilder: [{ identifikation: 'fictional-wage', beregningsfelt: v('Par14Bruttoløn'), delårsbeløb_kroner: 300000,
      omregningsmetode: v('Par14DokumenteretRetvisendeHelårsbeløb', { helårsbeløb_kroner: 600000 }),
      faktisk_helårsbeløb_kroner: null }],
    helårsgrundlag: v('DokumenteretHelårsPersonskat', { personskat: annual }) };
  const specs = [
    ['final-debt-intermediate-refund', true, true, true, () => {}],
    ['wrong-direction', false, true, false, i => { i.personskat.årsopgørelse.afregningsfakta = refund(); }],
    ['wrong-year', false, true, false, i => { i.personskat.årsopgørelse.afregningsfakta = debt(2024); }],
    ['invalid-payment-facts', false, true, false, i => { i.personskat.årsopgørelse.afregningsfakta.fakta.øvrige_skyldige_renter_øre = -1; }],
    ['final-refund', true, true, true, i => { i.personskat.årsopgørelse = settlement(12000000, refund()); }],
    // No payment instruction is not a claim that a payout was calculated.
    ['zero-balance-without-payment', true, true, true, i => { i.personskat.årsopgørelse = settlement(10375224, v('UdenSlutopgørelsesafregning')); }],
    ['duplicate-derived-credit', false, false, false, i => { i.personskat.årsopgørelse.kreditter.personskattelov_par8a_stk5_beløb_øre = 1; }],
    ['annual-source-credit', false, false, false, i => {
      i.helårsgrundlag.personskat.årsopgørelse = settlement(0, v('UdenSlutopgørelsesafregning'));
      i.helårsgrundlag.personskat.årsopgørelse.kreditter.personskattelov_par8a_stk5_beløb_øre = 1;
    }],
    ['negative-period-wage', false, false, false, i => { i.personskat.lønmodtager.bruttoløn_kroner = -1; }],
  ];
  const cases = specs.map(([case_id, valid, calculationValid, settlementValid, edit]) => {
    const input = structuredClone(base); input.personskat.årsopgørelse = settlement(10000000, debt()); edit(input);
    return { case_id, input, valid, calculationValid, settlementValid };
  });
  const results = call(partYear, envelope, cases);
  for (const c of cases) {
    const r = results.get(c.case_id);
    assessment(r.vurdering, c.valid, c.calculationValid, c.settlementValid);
    assert.equal(r.input_gyldigt, c.valid, c.case_id);
    assert.equal(r.slutskat_efter_par14_øre, c.calculationValid ? 10375224 : 0, c.case_id);
    assert.equal(r.vurdering.kontrolgrundlag.afregning[0].sti, 'personskat.årsopgørelse.afregningsfakta');
    if (c.valid) assert.equal(r.vurdering.slutskat_til_sammenligning_øre, 10375224);
    console.log(`${c.case_id}: valid=${c.valid}; final-tax-øre=${r.slutskat_efter_par14_øre}; settlement=${r.årsopgørelse.afregning.$variant}`);
  }
  const control = results.get('final-debt-intermediate-refund');
  assert.equal(control.delårsresultat.slutskat_øre, 9433144);
  assert.equal(control.delårsresultat.vurdering.alle_kontroller_gyldige, false);
  assert.ok(control.delårsresultat.vurdering.kontrolgrundlag.beregning.every(c => c.gyldig));
  assert.equal(control.årsopgørelse.afregning.$variant, 'RestskatAfregnet');
  assert.equal(control.årsopgørelse.afregning.resultat.restskat_øre, 375224);
  assert.equal(control.årsopgørelse.afregning.resultat.opkræves_kroner, 3966);
  assert.equal(results.get('wrong-direction').delårsresultat.vurdering.alle_kontroller_gyldige, true);
  for (const id of ['wrong-direction', 'wrong-year'])
    assert.equal(results.get(id).årsopgørelse.afregning.$variant, 'AfregningsfaktaMatcherIkkeSlutopgørelse');
  assert.equal(results.get('final-refund').årsopgørelse.afregning.resultat.overskydende_skat_øre, 1624776);
  assert.equal(results.get('zero-balance-without-payment').årsopgørelse.afregning.$variant, 'IngenSlutopgørelsesafregning');
});

test('green-check settlement keeps its post-credit stage and source controls', enabled, () => {
  const envelope = run(['template', greenCheck, '--format', 'json']);
  const p = person(envelope.cases[0].input.personskat);
  p.lønmodtager.bruttoløn_kroner = 0; p.lønmodtager.pension.fødselsdato.år = 1950;
  p.årsopgørelse = settlement(0, refund()); p.årsopgørelse.overført_restskat_mv_øre = 10000;
  const base = { personskat: p, oplyst_samlet_grøn_check_øre: null, grøn_check_fakta: {
    skattepligt_den_1_januar: v('GrønCheckFuldSkattepligt'), forskerordning: false,
    modtager_førtids_senior_eller_tidlig_pension_ved_årets_udløb: false,
    årsforløb: v('GrønCheckOrdinærtHelår'), samliv: v('GrønCheckIkkeSamlevendeVedÅrsslut'), pbl16_tillæg_kroner: 0,
    børneforhold: v('GrønCheckOplysteBørn', { børn: [], oplysninger_komplette: true, kildereference: 'fictional-no-children' }),
    kildereference: 'fictional-full-year-person' } };
  const specs = [
    ['debt-becomes-refund', true, true, true, () => {}],
    ['wrong-direction', false, true, false, i => { i.personskat.årsopgørelse.afregningsfakta = debt(); }],
    ['wrong-year', false, true, false, i => { i.personskat.årsopgørelse.afregningsfakta = refund(2024); }],
    ['source-credit-error', false, false, true, i => { i.personskat.årsopgørelse.kreditter.personskattelov_par8a_stk5_beløb_øre = 1; }],
  ];
  const cases = specs.map(([case_id, valid, calculationValid, settlementValid, edit]) => {
    const input = structuredClone(base); edit(input); return { case_id, input, valid, calculationValid, settlementValid };
  });
  const results = call(greenCheck, envelope, cases);
  for (const c of cases) {
    const r = results.get(c.case_id);
    assessment(r.vurdering, c.valid, c.calculationValid, c.settlementValid);
    if (!c.valid) assert.equal(r.årsopgørelse, null, c.case_id);
  }
  const r = results.get('debt-becomes-refund');
  assert.equal(r.vurdering.slutskat_til_sammenligning_øre, 0);
  assert.equal(r.grøn_check.beregning.samlet_kredit_øre, 128500);
  assert.equal(r.årsopgørelse.afregning.$variant, 'OverskydendeSkatAfregnet');
  assert.equal(r.årsopgørelse.afregning.resultat.overskydende_skat_øre, 118500);
  assert.equal(r.årsopgørelse.afregning.resultat.godtgørelsesgrundlag_øre, 0);
});
