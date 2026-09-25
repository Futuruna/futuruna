// Synthetic source facts only. No network or automatic compiler build.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const model = 'examples/danish-income-tax/personskat-groen-check.calculate.runa';
const entry = 'beregn_personskat_med_grøn_check';
const canonical = 'examples/danish-income-tax/personskat.calculate.runa';
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
const variant = ($variant, fields = {}) => ({ $variant, ...fields });
const date = (år) => ({ år, måned: 1, dag: 1 });
const creditNames = ['a_skat_og_am_indeholdt', 'par68_indbetalt', 'b_skat_betalt',
  'udbytteskat_modregningsberettiget', 'frivillig_indbetaling_par59', 'virksomhedsordning_beløb',
  'personskattelov_par8a_stk5_beløb', 'afskrivningslov_acontoskat', 'am_lov_par6_beløb',
  'seniornedslag', 'energiafgiftskompensation', 'tilbagebetalt_par55'];
const refund = (year) => variant('AfregnOverskydendeSkat', { fakta: {
  indkomstår: year, modsvarer_par59_indbetaling_øre: 0, tilbagebetalt_rente_par59_øre: 0,
  modsvarer_seniornedslag_øre: 0, modsvarer_energiafgiftskompensation_øre: 0,
  restancer_personlig_skat_med_morarenter_øre: 0, tilbagebetaling_efter_1_september: false,
  påbegyndte_måneder_efter_1_september: 0,
} });
const child = () => ({ lokal_reference: 'barn-1', fødselsdato: date(2010),
  ophold_den_1_januar: variant('GrønCheckBarnIDanmark'), har_indgået_ægteskab_den_1_januar: false,
  anbragt_døgnforanstaltning_eller_offentligt_forsørget_den_1_januar: false,
  børneydelse_ved_årets_udløb: variant('GrønCheckHelBørneydelse'), kildereference: 'syntetisk-ydelse',
});
const pensionPayment = () => ({
  identifikation: 'syntetisk-privat-rate', ordning: variant('Pbl18Rateopsparing'),
  indbetalingskilde: variant('Pbl18EgenIndbetaling'), fradragsretshaver: variant('Pbl18OrdningensEjer'),
  betaling: { beløb_kroner: 1000, forfaldsår: 2025, betalingsår: 2025,
    betalt_senest_bankjusteret_1_april_efter_forfald: true, hidrører_fra_par22e_tilbagebetaling: false,
    par15a_fradragsplacering: variant('Pbl18IkkePar15APlacering'), arbejdsmarkedsbidrag_kroner: 0 },
  fordelingsforløb: variant('Pbl18IngenTiårsfordeling'), indeksvalg: { fradragsvalgte_kontraktbidrag_kroner: [] },
  indeksordningsgrundlag: variant('Pbl18IkkeIndeksordning'), forfaldne_ikke_tidligere_fratrukket_kroner: 0,
  særligt_ordningsgrundlag: variant('Pbl18IntetSærligtOrdningsgrundlag'),
  begrænsninger: { pbl54_personkreds_opfyldt: true, afgiftspligt_for_hele_ordningen_indtrådt: false,
    udenlandsk_overførsel_med_tidligere_fradrag_uden_skatte_eller_afgiftskonsekvens: false },
});

function baseline(input) {
  const p = input.personskat;
  Object.assign(p.lønmodtager, { skatteår: 2025, bruttoløn_kroner: 300000,
    kommune: variant('København'), betaler_kirkeskat: false });
  p.lønmodtager.pension.fødselsdato = date(1950);
  p.lønmodtager.pension.atp = variant('IngenAtpIndbetalinger');
  p.lønmodtager.pension.udbetalingsoplysninger = { for_året_komplette: true, for_foregående_år_komplette: true };
  p.lønmodtager.ligningsfradrag.enlig_forsørger = variant('IntetEkstraBørnetilskud');
  p.lønmodtager.ligningsfradrag.boligjob = variant('IngenBoligjobudgifter');
  p.lønmodtager.ligningsfradrag.arbejdsfradrag_udland = variant('IngenUdlandsudelukkelseIFællesForhold', {
    dbo_hjemmehørende_udland_i_nogen_periode: false, noget_arbejde_udført_udland: null,
    nogen_udenlandsk_arbejdsgiver: null, kildereference: 'syntetisk-DK-helår',
  });
  // Empty/default branches are declared absent for this fictional person only.
  assert.deepEqual(p.ægtefælle, variant('UdenÆgtefælle'));
  p.årsopgørelse = variant('MedEksaktÅrsopgørelse', {
    overført_restskat_mv_øre: 0, øvrig_pensionsbeskatningsafgift_øre: 0,
    kreditter: Object.fromEntries(creditNames.map(n => [`${n}_øre`, 0])),
    afskrivningslov40c_acontoskat: variant('UdenAfskrivningslov40CAcontoskat'),
    afregningsfakta: variant('UdenSlutopgørelsesafregning'),
  });
  input.grøn_check_fakta = {
    skattepligt_den_1_januar: variant('GrønCheckFuldSkattepligt'), forskerordning: false,
    modtager_førtids_senior_eller_tidlig_pension_ved_årets_udløb: false,
    årsforløb: variant('GrønCheckOrdinærtHelår'), samliv: variant('GrønCheckIkkeSamlevendeVedÅrsslut'),
    pbl16_tillæg_kroner: 0,
    børneforhold: variant('GrønCheckOplysteBørn', { børn: [], oplysninger_komplette: true, kildereference: 'syntetisk-komplet-liste' }),
    kildereference: 'syntetiske-personfakta',
  };
  input.oplyst_samlet_grøn_check_øre = null;
  return input;
}
function spouse(input) {
  const p = input.personskat;
  const fields = ['lønmodtager', 'kapitalindkomst', 'aktieavance', 'udenlandske_sociale_bidrag',
    'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter'];
  const fakta = Object.fromEntries(fields.map(n => [n, structuredClone(p[n])]));
  fakta.lønmodtager.bruttoløn_kroner = 0;
  p.ægtefælle = variant('MedÆgtefælle', { fakta, samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] });
  input.grøn_check_fakta.samliv = variant('GrønCheckSamlevendeHeleÅret');
}

test('integrated green check uses the canonical engine and explicit coverage gates', () => {
  const s = readFileSync(join(root, model), 'utf8');
  assert.match(s, /beregn_personskat\(personskat_grøn_check_uden_afregning/);
  assert.match(s, /vurder_grøn_check_input\(grundlag\)/);
  assert.match(s, /personskat_afregning_input_gyldigt\(a\)/);
  assert.match(s, /årsopgørelse: PersonskatÅrsopgørelseResultat\?/);
  assert.doesNotMatch(s, /@ rust/);
});

test('canonical derived-income green-check settlement and fail-closed boundaries', {
  skip: binary ? false : 'set FUTURUNA_MODEL_TEST_RUNA; no compiler build or network',
}, () => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-personskat-green-check-'));
  const save = (name, value) => {
    const path = join(directory, name);
    writeFileSync(path, `${JSON.stringify(value)}\n`, { mode: 0o600, flag: 'wx' });
    return path;
  };
  const run = args => {
    const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 600000,
      maxBuffer: 32 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
    assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
  };
  console.log(`Synthetic canonical green-check evidence: ${directory}`);
  const envelope = run(['template', model, '--entry', entry, '--format', 'json']);
  const base = baseline(envelope.cases[0].input);
  const original = run(['template', canonical, '--format', 'json']);
  const manual = structuredClone(base.personskat);
  manual.årsopgørelse.kreditter.energiafgiftskompensation_øre = 128500;
  original.cases = [{ case_id: 'legacy-zero', input: base.personskat }, { case_id: 'legacy-external', input: manual }];
  const prior = run(['call', canonical, '--input', save('canonical-input.json', original)]);
  save('canonical-results.json', prior);
  assert.deepEqual(prior.diagnostics, []);
  const unchanged = prior.results[0].result;
  assert.equal(unchanged.vurdering.alle_kontroller_gyldige, true);
  const tax = unchanged.vurdering.slutskat_til_sammenligning_øre;

  const cases = [];
  const add = (case_id, edit = () => {}, credit = 128500, valid = true) => {
    const input = structuredClone(base); edit(input);
    cases.push({ case_id, input, credit, valid });
  };
  add('derived-income');
  add('wage-removes-supplement', i => { i.personskat.lønmodtager.bruttoløn_kroner = 302000; }, 100500);
  add('pension-restores-supplement', i => { i.personskat.lønmodtager.bruttoløn_kroner = 302000; i.personskat.lønmodtager.pension.pbl18_indbetalinger.push(pensionPayment()); });
  add('2026-derived-credit', i => { i.personskat.lønmodtager.skatteår = 2026; }, 115500);
  add('capital-removes-supplement', i => { i.personskat.kapitalindkomst.renter.renteindtægter_kroner = 55000; }, 100500);
  add('known-young-zero', i => { i.personskat.lønmodtager.pension.fødselsdato = date(1990); }, 0);
  add('child-whole', i => { i.personskat.lønmodtager.pension.fødselsdato = date(1990); i.grøn_check_fakta.børneforhold.børn = [child()]; }, 24000);
  add('observed-credit-does-not-control-result', i => { i.oplyst_samlet_grøn_check_øre = 1; });
  add('debt-becomes-refund', i => { i.personskat.årsopgørelse.kreditter.a_skat_og_am_indeholdt_øre = tax - 10000; i.personskat.årsopgørelse.afregningsfakta = refund(2025); });
  add('2023-interest-excludes-credit', i => {
    i.personskat.lønmodtager.skatteår = 2023; i.personskat.lønmodtager.bruttoløn_kroner = 0;
    i.personskat.årsopgørelse.kreditter.a_skat_og_am_indeholdt_øre = 100000;
    i.personskat.årsopgørelse.afregningsfakta = refund(2023);
  }, 115500);
  add('2023-only-part-credit-refunded', i => {
    i.personskat.lønmodtager.skatteår = 2023; i.personskat.lønmodtager.bruttoløn_kroner = 0;
    i.personskat.årsopgørelse.overført_restskat_mv_øre = 100000;
    i.personskat.årsopgørelse.afregningsfakta = refund(2023);
  }, 115500);
  add('spouse-derived-capital', i => { spouse(i); i.personskat.ægtefælle.fakta.kapitalindkomst.renter.renteudgifter_kroner = 30000; });
  for (const [id, edit] of [
    ['unknown-children', i => { i.grøn_check_fakta.børneforhold = variant('GrønCheckBørnUoplyst'); }],
    ['unknown-researcher', i => { i.grøn_check_fakta.forskerordning = null; }],
    ['unknown-pbl16', i => { i.grøn_check_fakta.pbl16_tillæg_kroner = null; }],
    ['positive-pbl16', i => { i.grøn_check_fakta.pbl16_tillæg_kroner = 100; }],
    ['canonical-missing-facts', i => { i.personskat.lønmodtager.ligningsfradrag.boligjob = variant('BoligjobUoplyst'); }],
    ['duplicate-credit', i => { i.personskat.årsopgørelse.kreditter.energiafgiftskompensation_øre = 128500; }],
    ['duplicate-refund-exclusion', i => { i.personskat.årsopgørelse.afregningsfakta = refund(2025); i.personskat.årsopgørelse.afregningsfakta.fakta.modsvarer_energiafgiftskompensation_øre = 128500; }],
    ['wrong-payment-direction', i => { i.personskat.årsopgørelse.afregningsfakta = refund(2025); }],
    ['wrong-payment-year', i => { i.personskat.årsopgørelse.kreditter.a_skat_og_am_indeholdt_øre = tax; i.personskat.årsopgørelse.afregningsfakta = refund(2024); }],
    ['part-year', i => { i.grøn_check_fakta.årsforløb = variant('GrønCheckAndetÅrsforløb'); }],
    ['conflicting-spouse', i => { spouse(i); i.grøn_check_fakta.samliv = variant('GrønCheckIkkeSamlevendeVedÅrsslut'); }],
    ['spouse-positive-capital-unresolved', i => { spouse(i); i.personskat.ægtefælle.fakta.kapitalindkomst.renter.renteindtægter_kroner = 110000; }],
    ['no-settlement', i => { i.personskat.årsopgørelse = variant('UdenÅrsopgørelse'); }],
  ]) add(id, edit, null, false);
  envelope.cases = cases.map(({ case_id, input }) => ({ case_id, input }));
  const output = run(['call', model, '--entry', entry, '--input', save('input.json', envelope)]);
  save('results.json', output);
  assert.deepEqual(output.diagnostics, []);
  assert.equal(output.results.length, cases.length);
  const byId = new Map(output.results.map(c => [c.case_id, c.result]));
  for (const c of cases) {
    const r = byId.get(c.case_id); assert.ok(r, c.case_id);
    assert.equal(r.vurdering.alle_kontroller_gyldige, c.valid, `${c.case_id}: ${JSON.stringify(r.vurdering.fejl)}`);
    assert.equal(r.vurdering.samlet_modeldækning_bekræftet, false, c.case_id);
    if (c.valid) {
      assert.equal(r.grøn_check.beregning.samlet_kredit_øre, c.credit, c.case_id);
      assert.equal(r.årsopgørelse.input.kreditter.energiafgiftskompensation_øre, c.credit, c.case_id);
      assert.equal(r.årsopgørelse.input.slutskat_øre, r.vurdering.slutskat_til_sammenligning_øre, c.case_id);
    } else {
      assert.equal(r.årsopgørelse, null, c.case_id);
      assert.equal(r.vurdering.slutskat_til_sammenligning_øre, null, c.case_id);
      assert.ok(r.vurdering.fejl.length > 0, c.case_id);
    }
  }
  assert.deepEqual(byId.get('derived-income').årsopgørelse, prior.results[1].result.årsopgørelse);
  assert.equal(byId.get('derived-income').grøn_check_grundlag.indkomst.personlig_indkomst_efter_am_kroner, 276000);
  assert.equal(byId.get('pension-restores-supplement').grøn_check_grundlag.indkomst.personlig_indkomst_efter_am_kroner, 276840);
  assert.equal(byId.get('capital-removes-supplement').grøn_check_grundlag.indkomst.nettokapitalindkomst_kroner, 55000);
  assert.equal(byId.get('spouse-derived-capital').grøn_check_grundlag.ægtefælle.ægtefælles_nettokapitalindkomst_kroner, -30000);
  assert.equal(byId.get('observed-credit-does-not-control-result').grøn_check.difference_øre, -128499);
  assert.equal(byId.get('observed-credit-does-not-control-result').grøn_check.status.$variant, 'GrønCheckAfviger');
  const flip = byId.get('debt-becomes-refund').årsopgørelse;
  assert.equal(flip.resultat.overskydende_skat_øre, 118500);
  assert.equal(flip.afregning.resultat.godtgørelsesgrundlag_øre, 0);
  const interest = byId.get('2023-interest-excludes-credit').årsopgørelse.afregning.resultat;
  assert.equal(interest.overskydende_skat_øre, 215500);
  assert.equal(interest.godtgørelsesgrundlag_øre, 100000);
  assert.equal(interest.godtgørelse_øre, 800);
  const partial = byId.get('2023-only-part-credit-refunded').årsopgørelse.afregning.resultat;
  assert.equal(partial.overskydende_skat_øre, 15500);
  assert.equal(partial.godtgørelsesgrundlag_øre, 0);
  assert.equal(partial.godtgørelse_øre, 0);
  for (const id of ['unknown-children', 'unknown-researcher', 'unknown-pbl16', 'canonical-missing-facts', 'spouse-positive-capital-unresolved']) {
    assert.equal(byId.get(id).grøn_check.beregning, null, id);
  }
  console.log(`Validated ${cases.length} integrated cases and two legacy comparisons.`);
});
