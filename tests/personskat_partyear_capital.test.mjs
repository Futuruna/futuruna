// Source-model composition checks, NOT an independent SKAT part-year oracle.
// Fictional arrival-year couples with separately documented annual amounts.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { buildFictionalCases } from '../examples/danish-income-tax/bilag-demo.mjs';
import { personFactFields } from '../examples/danish-income-tax/pension-par-demo.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
const model = 'examples/danish-income-tax/personskat-par14.calculate.runa';
const enabled = { skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no build or network.' };
const v = ($variant, fields = {}) => ({ $variant, ...fields });
function run(args) {
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 1200000,
    maxBuffer: 64 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
}
function save(directory, name, value) {
  const file = join(directory, name);
  writeFileSync(file, JSON.stringify(value), { flag: 'wx', mode: 0o600 }); return file;
}
const source = (field, period, annual) => ({
  identifikation: `fictional-${field}`, beregningsfelt: v(field), delårsbeløb_kroner: period,
  omregningsmetode: v('Par14DokumenteretRetvisendeHelårsbeløb', { helårsbeløb_kroner: annual }),
  faktisk_helårsbeløb_kroner: null,
});

test('part-year bundskat and PSL11 retain each documented spouse capital context', enabled, () => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-partyear-capital-canonical-'));
  console.log(`Fictional part-year capital evidence: ${directory}`);
  const envelope = run(['template', model, '--format', 'json']);
  const base = buildFictionalCases({ cases: [{ input: envelope.cases[0].input.personskat }] })
    .envelope.cases[0].input;
  base.lønmodtager.pension.pbl18_indbetalinger = [];
  // Reuse construction, not the demo's full-year source ledger. Both persons
  // are born 1990, Copenhagen, no church/ATP/pension/property/losses/relief or
  // other amounts. Full liability starts July1; qualifying cohabitation at
  // year-end is an explicit fact, not inferred from a shared address.
  // Annual amounts below are separately documented representative amounts
  // under PSL14(1), not a blanket day-factor or an actual-annual-income election.
  function person(wage, capital) {
    const p = structuredClone(base);
    p.lønmodtager.bruttoløn_kroner = wage;
    p.kapitalindkomst.renter.renteindtægter_kroner = Math.max(capital, 0);
    p.kapitalindkomst.renter.renteudgifter_kroner = Math.max(-capital, 0);
    return p;
  }
  function couple(ownWage, ownCapital, spouseCapital, cohabits) {
    const p = person(ownWage, ownCapital), partner = person(300000, spouseCapital);
    p.ægtefælle = v('MedÆgtefælle', {
      fakta: Object.fromEntries(personFactFields.map(key => [key, partner[key]])),
      samlevende_ved_indkomstårets_udløb: cohabits, kildeskat25a_fordelinger: [],
    });
    return p;
  }
  function input(ownCapital, spouseCapital, cohabits = true) {
    const period = couple(200000, ownCapital[0], spouseCapital[0], cohabits);
    const annual = couple(400000, ownCapital[1], spouseCapital[1], cohabits);
    annual.ægtefælle.fakta.lønmodtager.bruttoløn_kroner = 600000;
    return { personskat: period, skattepligtsændring: v('FuldSkattepligtIndtræder'),
      skattepligtsperiode: { fra_dato: { år: 2025, måned: 7, dag: 1 }, til_dato: { år: 2025, måned: 12, dag: 31 } },
      valg_afgivet_ved_oplysninger: false, omvalg_dato: null,
      kilder: [source('Par14Bruttoløn', 200000, 400000), source('Par14Nettokapitalindkomst', ...ownCapital)],
      helårsgrundlag: v('DokumenteretHelårsPersonskat', { personskat: annual }) };
  }
  const cases = [
    { case_id: 'partial-offset', input: input([20000, 40000], [-10000, -15000]) },
    { case_id: 'no-cohabitation', input: input([20000, 40000], [-10000, -15000], false) },
    { case_id: 'negative-capital', input: input([-60000, -120000], [20000, 40000]) },
  ];
  const single = input([0, 0], [0, 0]);
  single.personskat.ægtefælle = v('UdenÆgtefælle');
  single.personskat.lønmodtager.bruttoløn_kroner = 300000;
  single.skattepligtsændring = v('FuldSkattepligtOphører');
  single.skattepligtsperiode = { fra_dato: { år: 2025, måned: 1, dag: 1 }, til_dato: { år: 2025, måned: 6, dag: 30 } };
  single.helårsgrundlag = v('AfledtFraIdentificeredeKilder');
  single.kilder = [{ ...source('Par14Bruttoløn', 300000, 0), omregningsmetode: v('Par14ForholdsmæssigtLøbendeBeløb') }];
  cases.push({ case_id: 'single-control', input: single });
  envelope.cases = cases;
  const output = run(['call', model, '--input', save(directory, 'input.json', envelope)]);
  save(directory, 'results.json', output);
  assert.deepEqual(output.diagnostics, []);
  assert.deepEqual(output.results.map(row => row.case_id), cases.map(row => row.case_id));
  for (const { case_id, result: r } of output.results) {
    console.log(JSON.stringify({ case_id, valid: r.input_gyldigt, faults: r.vurdering.fejl,
      bundskat: r.statslige_skattekomponenter.find(c => c.art.$variant === 'Par14Bundskat'),
      par11: r.par11, comparison: r.vurdering.slutskat_til_sammenligning_øre }));
  }
  const results = new Map(output.results.map(row => [row.case_id, row.result]));
  for (const [case_id, r] of results) {
    assert.equal(r.input_gyldigt, true, case_id);
    assert.equal(r.vurdering.alle_kontroller_gyldige, true, case_id);
    assert.equal(r.vurdering.slutskat_til_sammenligning_øre, r.slutskat_efter_par14_øre, case_id);
    assert.equal(r.vurdering.samlet_modeldækning_bekræftet, false, case_id);
  }
  for (const [id, numerator, denominator] of [
    ['partial-offset', 194000, 393000], ['no-cohabitation', 204000, 408000],
  ]) {
    const r = results.get(id), c = r.statslige_skattekomponenter.find(c => c.art.$variant === 'Par14Bundskat');
    // PSL6(3) offset BEFORE the matching period/annual PSL14 ratio. Source
    // capital amounts remain 20000/40000; they are NOT rewritten to the offset.
    assert.equal(c.nedsættelsesfaktor_tæller_kroner, numerator, id);
    assert.equal(c.nedsættelsesfaktor_nævner_kroner, denominator, id);
    assert.equal(c.helårsskat_øre, r.helårsberegning_eksakt.efter_personfradrag.par6_skat_øre, id);
    assert.equal(c.delårsskat_øre, Number(BigInt(c.helårsskat_øre) * BigInt(numerator) / BigInt(denominator)), id);
    assert.equal(r.delårsinput.nettokapitalindkomst_kroner, 20000, id);
    assert.equal(r.helårsinput.nettokapitalindkomst_kroner, 40000, id);
    assert.equal(r.statslig_indkomstskat_efter_par14_øre, c.delårsskat_øre, id);
  }
  const negative = results.get('negative-capital');
  for (const [basis, offset, net, credit] of [
    ['delårs', 20000, 40000, 320000], ['helårs', 40000, 80000, 640000],
  ]) {
    const p = negative.par11[`${basis}grundlag`];
    assert.equal(p.ægtefælle_positiv_modregning_kroner, offset, basis);
    assert.equal(p.negativ_nettokapitalindkomst_efter_ægtefællepositiv_kroner, net, basis);
    assert.equal(p.effektiv_beløbsgrænse_kroner, 100000, basis);
    assert.equal(negative.par11[`${basis}_beregnet_nedslag_øre`], credit, basis);
    assert.equal(negative.par11[`${basis}_anvendt_nedslag_i_kanonisk_beregning_øre`], credit, basis);
  }
  // Recorded component/control totals, not independent administrative results.
  assert.equal(results.get('partial-offset').vurdering.slutskat_til_sammenligning_øre, 7228683);
  assert.equal(results.get('no-cohabitation').vurdering.slutskat_til_sammenligning_øre, 7344841);
  assert.equal(negative.vurdering.slutskat_til_sammenligning_øre, 4900606);
  assert.equal(results.get('single-control').vurdering.slutskat_til_sammenligning_øre, 10383101);
});

test('generated annual-basis guidance keeps own and spouse capital facts separate', enabled, () => {
  const schema = run(['schema', model, '--format', 'compact-json']);
  const fields = schema.field_metadata.filter(f => f.path === 'helårsgrundlag.$variant');
  assert.equal(fields.length, 1);
  const field = fields[0];
  for (const phrase of ['egne kapitalbeløb før ægtefællemodregning',
    'delårs- og helårsbeløb skal dokumenteres særskilt', 'udled ikke beløbet af den forventede skat']) {
    assert.ok(field.help.includes(phrase), phrase);
  }
  const sources = schema.source_groups[field.source_group].map(id => schema.source_objects[id]);
  assert.ok(sources.some(s => s.role === 'source' && JSON.stringify(s).includes('2021/1284')));
  assert.ok(sources.some(s => s.role === 'guidance' && JSON.stringify(s).includes('oid=1977388')));
});
