// Fictional service invoices, not personal documents or observed SKAT totals.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
const compact = 'examples/danish-income-tax/boligjob.calculate.runa';
const canonical = 'examples/danish-income-tax/personskat.calculate.runa';
const options = { skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no build or network.' };
const v = ($variant, fields = {}) => ({ $variant, ...fields });
const date = (år, måned = 12, dag = 31) => ({ år, måned, dag });
const source = 'https://skat.dk/borger/fradrag/servicefradrag/salg-af-serviceydelser-med-servicefradrag-privatperson';

function evidence() {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-boligjob-age-'));
  console.log(`Fictional supplier-age evidence: ${directory}`);
  return (name, value) => {
    const path = join(directory, name);
    writeFileSync(path, JSON.stringify(value) + '\n', { flag: 'wx', mode: 0o600 }); return path;
  };
}
function run(args) {
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 600000,
    maxBuffer: 32 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
}
function expenses(workYear = 2024, born = date(2007), payment = date(2025, 3, 1), person = 'hovedperson') {
  return v('OplysteBoligjobudgifter', {
    oplysninger_for_året_komplette: true, skattepligt: v('Ll8VFuldtSkattepligtig'), personreference: person,
    poster: [{ betaling: {
      identifikation: 'fiktiv-betalingslinje', kildereference: 'fiktiv-serviceerklæring-linje-1',
      betalingsreference: 'fiktiv-overførsel', boligreference: 'fiktiv-helårsbolig',
      ydelse: v('Ll8VRengøring'), arbejdsdato: date(workYear, 12, 1), betalingsdato: payment,
      land: v('Ll8VDanmark'), eksisterende_bolig: v('Ll8VJa'),
      leverandør: v('Ll8VPrivatperson', { fødselsdato: born, fuld_skattepligt_i: v('Ll8VDanskRegistrering') }),
      betalt_arbejdsløn_øre: 300000, betalte_materialer_kørsel_m_v_øre: 0,
      betalingsform: v('Ll8VKontooverførsel'), arbejdsbilag_med_påkrævede_oplysninger: v('Ll8VJa'),
      betalingsbilag: v('Ll8VJa'), offentligt_tilskud: v('Ll8VNej'), samme_udgift_fradraget_efter_andre_regler: v('Ll8VNej'),
      børnepasning_skattefri_efter_par7æ: v('Ll8VNej'),
      udfører_bor_i_helårsboligen_eller_ejer_fritidsboligen_eller_bor_med_ejer: v('Ll8VNej'),
      betalerreference: person, andele: [{ personreference: person, arbejdsløn_øre: 300000 }],
      særligt_forhold: v('Ll8VAlmindeligUdgift'),
    }, boligforhold: v('Ll8VHelårsbolig', { fast_bopæl_ved_arbejdet: v('Ll8VJa'), individuel_råderet_og_vedligeholdelsesret: v('Ll8VJa') }),
    fællesøkonomi_med_betalende_ægtefælle_eller_samlever: v('Ll8VUoplyst'), indberettet_med_leverandøroplysninger: v('Ll8VJa') }],
  });
}
function checkMetadata(schema, prefixes) {
  for (const prefix of prefixes) {
    for (const [suffix, phrase] of [
      ['$variant', 'privatperson'],
      ['Ll8VPrivatperson.fødselsdato.år', 'arbejdsårets'],
      ['Ll8VPrivatperson.fødselsdato.måned', 'udføreren'],
      ['Ll8VPrivatperson.fødselsdato.dag', 'udføreren'],
    ]) {
      const path = `${prefix}.leverandør.${suffix}`;
      const fields = schema.field_metadata.filter(field => field.path === path);
      assert.equal(fields.length, 1, path);
      const field = fields[0];
      assert.equal(field.anchor, 'Ll8VLeverandør');
      assert.ok(field.question?.length > 0, path);
      assert.ok(field.help?.includes(phrase), `${path}: ${phrase}`);
      const sources = schema.source_groups[field.source_group]?.map(id => schema.source_objects[id]);
      assert.ok(JSON.stringify(sources).includes(source), `${path}: supplier source trace`);
    }
  }
}

test('compact supplier age follows the work year, not the payment or selected year', options, () => {
  const save = evidence();
  const envelope = run(['template', compact, '--format', 'json']);
  const cases = [];
  const add = (case_id, year, input, expected, valid = true, claimant = date(1980)) => {
    cases.push({ case_id, input: { indkomstår: year, fødselsdato: claimant, udgifter: input }, expected, valid });
  };
  add('too-young-2024-paid-march-2025', 2025, expenses(), 0);
  add('adult-by-work-year-end', 2025, expenses(2024, date(2006)), 300000);
  add('too-young-2025-paid-march-2026', 2026, expenses(2025, date(2008), date(2026, 3, 1)), 0);
  add('same-year-turns-18-after-work', 2025, expenses(2025, date(2007), date(2025, 12, 15)), 300000);
  add('february-payment-stays-2024', 2024, expenses(2024, date(2006), date(2025, 2, 28)), 300000);
  add('february-not-deductible-in-2025', 2025, expenses(2024, date(2006), date(2025, 2, 28)), 0);
  add('claimant-turns-18-in-deduction-year', 2025, expenses(2024, date(1980)), 300000, true, date(2007));
  add('claimant-under-18-at-deduction-year-end', 2025, expenses(2024, date(1980)), 0, true, date(2008));
  const company = expenses(); company.poster[0].betaling.leverandør = v('Ll8VVirksomhed', {
    moms_eller_tredjelandsregistrering: v('Ll8VDanskRegistrering'),
    udenlandsk_virksomhed_i_danmark: v('Ll8VNej'), rut_registreret: v('Ll8VUoplyst'),
  });
  add('company-unchanged', 2025, company, 300000);
  const unknown = expenses(); unknown.poster[0].betaling.leverandør = v('Ll8VUoplystLeverandør');
  add('unknown-provider', 2025, unknown, 0, false);
  add('impossible-provider-birthday', 2025, expenses(2024, date(2006, 2, 30)), 0, false);
  add('provider-born-after-work-year', 2025, expenses(2024, date(2025, 1, 1)), 0, false);
  envelope.cases = cases.map(({ case_id, input }) => ({ case_id, input }));
  const output = run(['call', compact, '--input', save('compact-cases.json', envelope)]);
  save('compact-results.json', output);
  assert.deepEqual(output.diagnostics, []); assert.equal(output.results.length, cases.length);
  console.log(JSON.stringify(output.results.map(({ case_id, result: r }) => ({ case_id, valid: r.input_gyldigt, deduction_ore: r.servicefradrag_øre }))));
  for (const [i, { case_id, result: r }] of output.results.entries()) {
    const c = cases[i]; assert.equal(case_id, c.case_id);
    assert.equal(r.input_gyldigt, c.valid, case_id);
    assert.equal(r.servicefradrag_øre, c.expected, case_id);
  }
  const young = output.results[0].result.poster[0];
  assert.equal(young.fradragsår, 2025);
  assert.equal(young.betingelser_opfyldt, false);
  assert.ok(young.kontrolpunkter.some(message => message.includes('arbejdsårets')));
  const schema = run(['schema', compact, '--format', 'compact-json']);
  checkMetadata(schema, ['udgifter.OplysteBoligjobudgifter.poster.betaling']);
});

test('canonical supplier-age correction reaches taxpayer and spouse without invalidating lawful facts', options, () => {
  const save = evidence();
  const envelope = run(['template', canonical, '--format', 'json']);
  const base = envelope.cases[0].input;
  base.ægtefælle = { $variant: 'UdenÆgtefælle' }; // Explicit fictional absence.
  // Every absent branch describes this invented whole-year Danish adult only.
  // Empty template values are not evidence of absence for a real taxpayer.
  Object.assign(base.lønmodtager, { skatteår: 2025, bruttoløn_kroner: 600000,
    kommune: v('København'), kirkeskat: { $variant: 'IngenKirkeskatHeleÅret' } });
  Object.assign(base.lønmodtager.pension, { fødselsdato: date(1980), atp: v('IngenAtpIndbetalinger'),
    udbetalingsoplysninger: { for_året_komplette: true, for_foregående_år_komplette: true } });
  Object.assign(base.lønmodtager.ligningsfradrag, {
    boligjob: v('IngenBoligjobudgifter'), enlig_forsørger: v('IntetEkstraBørnetilskud'),
    arbejdsfradrag_udland: v('IngenUdlandsudelukkelseIFællesForhold', {
      dbo_hjemmehørende_udland_i_nogen_periode: false, noget_arbejde_udført_udland: null,
      nogen_udenlandsk_arbejdsgiver: null, kildereference: 'Fiktiv DBO-hjemmehørende DK hele året',
    }),
  });
  const cases = [];
  const add = (case_id, born, expected, spouse = false) => {
    const input = structuredClone(base); let person = input;
    if (spouse) {
      const fields = ['lønmodtager', 'kapitalindkomst', 'aktieavance', 'udenlandske_sociale_bidrag',
        'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter'];
      person = Object.fromEntries(fields.map(f => [f, structuredClone(input[f])]));
      input.ægtefælle = v('MedÆgtefælle', { fakta: person, samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] });
    }
    if (born !== null) person.lønmodtager.ligningsfradrag.boligjob = expenses(2024, date(born), date(2025, 3, 1), spouse ? 'ægtefælle' : 'hovedperson');
    cases.push({ case_id, input, expected, spouse });
  };
  add('baseline', null, 0);
  add('young-provider', 2007, 0);
  add('adult-provider', 2006, 300000);
  add('spouse-young-provider', 2007, 0, true);
  add('spouse-adult-provider', 2006, 300000, true);
  envelope.cases = cases.map(({ case_id, input }) => ({ case_id, input }));
  const output = run(['call', canonical, '--input', save('canonical-cases.json', envelope)]);
  save('canonical-results.json', output);
  assert.deepEqual(output.diagnostics, []); assert.equal(output.results.length, cases.length);
  for (const [i, { case_id, result: r }] of output.results.entries()) {
    const c = cases[i], person = c.spouse ? r.ægtefælle.grundlag : r;
    assert.equal(case_id, c.case_id);
    assert.equal(r.vurdering.alle_kontroller_gyldige, true, case_id);
    assert.equal(r.vurdering.slutskat_til_sammenligning_øre, r.slutskat_øre, case_id);
    assert.equal(person.ligningsfradrag.boligjob.servicefradrag_øre, c.expected, case_id);
  }
  const results = Object.fromEntries(output.results.map(row => [row.case_id, row.result]));
  assert.equal(results.baseline.slutskat_øre, results['young-provider'].slutskat_øre);
  assert.equal(results['young-provider'].slutskat_øre - results['adult-provider'].slutskat_øre, 70500);
  const schema = run(['schema', canonical, '--format', 'compact-json']);
  checkMetadata(schema, ['', 'ægtefælle.MedÆgtefælle.fakta.'].map(prefix =>
    `${prefix}lønmodtager.ligningsfradrag.boligjob.OplysteBoligjobudgifter.poster.betaling`));
  console.log(JSON.stringify(output.results.map(({ case_id, result }) => ({ case_id,
    comparison_ore: result.vurdering.slutskat_til_sammenligning_øre }))));
});
