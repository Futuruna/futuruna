// Fictional source-model composition checks, not an independent SKAT oracle.
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
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr || p.stdout); return JSON.parse(p.stdout);
}
function save(directory, name, value) {
  const file = join(directory, name);
  writeFileSync(file, JSON.stringify(value), { flag: 'wx', mode: 0o600 }); return file;
}

test('part-year share tax retains spouse thresholds and netting without rewriting source income', enabled, async t => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-partyear-shares-canonical-'));
  console.log(`Fictional part-year share evidence: ${directory}`);
  const envelope = run(['template', model, '--format', 'json']);
  const base = buildFictionalCases({ cases: [{ input: envelope.cases[0].input.personskat }] })
    .envelope.cases[0].input;
  base.lønmodtager.pension.pbl18_indbetalinger = [];
  // Reuse construction, NOT the demo source ledger. Both people: born1990,
  // Copenhagen2025, no church/ATP/pension/capital/property/loss carryforward/
  // relief. Full liability begins July1. Salary200000/400000 own and
  // 300000/600000 spouse in period/documented representative annual basis.
  // All share income is a one-off amount within the Danish liability period:
  // no day-factor applied (DJV C.F.1.6.2.2), no foreign pre-arrival share facts.
  function person(wage, shares) {
    const p = structuredClone(base);
    p.lønmodtager.bruttoløn_kroner = wage;
    if (shares > 0) p.aktieavance.udbytter = [{ identifikation: 'fictional-dividend',
      udlodder: v('Ll16AAlmindeligtSelskab'), modtager: v('Ll16AAktuelAktionær'),
      aktiv: v('PersonskatAlmindeligAktie'), beløb_kroner: shares,
      par13a_kildefakta: v('AblPar13AUdbytteUdenForModregningsgrundlag') }];
    if (shares < 0) {
      const amount = v('AblAktiekapitalUdenPålydendeVærdi', { antal_aktier: 10 });
      // Unlisted shares acquired after arrival and fully sold at a loss;
      // explicitly no tax-free dividends or other ABL5A reduction facts.
      p.aktieavance.ordinært_aktieår = v('MedOrdinærtAktieår', { input: {
        indkomstår: 2025, rettighedsforløb: [], investeringsbeviser: [],
        aktiebaserede_minimumsbevis_forløb: [], fremført_tab_efter_par13a_kroner: 0,
        hændelsesforløb: [{ position_primo: { selskabsidentifikation: 'fictional-unlisted',
          kapitalmængde: v('AblAktiekapitalUdenPålydendeVærdi', { antal_aktier: 0 }), anskaffelsessum_kroner: 0 },
        hændelser: [
          { identifikation: 'buy', dato: { år: 2025, måned: 8, dag: 1 }, rækkefølge_på_dagen: 1,
            hændelse: v('AblOrdinærAnskaffelse', { kapitalmængde: amount, anskaffelsessum_kroner: 100000 }) },
          { identifikation: 'sell', dato: { år: 2025, måned: 10, dag: 1 }, rækkefølge_på_dagen: 1,
            hændelse: v('AblOrdinærAfståelse', { kapitalmængde: amount, afståelsessum_kroner: 100000 + shares,
              vilkår: { markedsstatus: v('AblIkkeOptagetTilHandel'), har_tidligere_været_optaget_til_handel: false,
                hovedaktionæraktier: false, afståede_aktiers_handelsværdi_kroner: 100000 + shares,
                beholdte_aktiers_handelsværdi_kroner: 0, oplysningsstatus: v('AblOplystRettidigt'), boligret: v('AblUdenBoligret') },
              par5a_kildefakta: v('AblOrdinærPar5AKildefakta', { fakta: {
                anvendelsesgrundlag: v('AblPar5AAfståelseDen24November2010EllerSenere'),
                skatteydergrundlag: v('AblPar5APersonSkattepligtigEfterPar7'), ejertidsudbytter: [], koncernbeløb: [],
                præferenceposition: { modtaget_tilsvarende_udbytte_kroner: 0, allerede_anvendt_til_tabsreduktion_kroner: 0 },
              } }) }) },
        ] }],
      } });
    }
    return p;
  }
  function input(ownShares, spouseShares, cohabits = true, withSpouse = true) {
    function couple(wage, spouseWage) {
      const p = person(wage, ownShares), partner = person(spouseWage, spouseShares);
      if (withSpouse) p.ægtefælle = v('MedÆgtefælle', {
        fakta: Object.fromEntries(personFactFields.map(key => [key, partner[key]])),
        samlevende_ved_indkomstårets_udløb: cohabits, kildeskat25a_fordelinger: [],
      });
      return p;
    }
    return { personskat: couple(200000, 300000), skattepligtsændring: v('FuldSkattepligtIndtræder'),
      skattepligtsperiode: { fra_dato: { år: 2025, måned: 7, dag: 1 }, til_dato: { år: 2025, måned: 12, dag: 31 } },
      valg_afgivet_ved_oplysninger: false, omvalg_dato: null,
      kilder: [
        { identifikation: 'salary', beregningsfelt: v('Par14Bruttoløn'), delårsbeløb_kroner: 200000,
          omregningsmetode: v('Par14DokumenteretRetvisendeHelårsbeløb', { helårsbeløb_kroner: 400000 }), faktisk_helårsbeløb_kroner: null },
        { identifikation: 'shares', beregningsfelt: v('Par14Aktieindkomst'), delårsbeløb_kroner: ownShares,
          omregningsmetode: v('Par14UændretEngangsbeløb'), faktisk_helårsbeløb_kroner: null },
      ], helårsgrundlag: v('DokumenteretHelårsPersonskat', { personskat: couple(400000, 600000) }) };
  }
  const cases = [
    { case_id: 'unused-threshold', input: input(100000, 0), net: 100000, low: 2700000, high: 0, total: 9334016 },
    { case_id: 'partial-threshold', input: input(100000, 40000), net: 100000, low: 2565000, high: 210000, total: 9409016 },
    { case_id: 'opposite-sign', input: input(100000, -40000), net: 60000, low: 1620000, high: 0, total: 8254016 },
    { case_id: 'fully-netted', input: input(20000, -20000), net: 0, low: 0, high: 0, total: 6634016 },
    { case_id: 'no-cohabitation', input: input(100000, 0, false), net: 100000, low: 1822500, high: 1365000, total: 9821516 },
    { case_id: 'single', input: input(100000, 0, false, false), net: 100000, low: 1822500, high: 1365000, total: 9821516 },
  ];
  for (const cohabits of [true, false]) {
    const x = input(0, 0, cohabits);
    x.helårsgrundlag.personskat.ægtefælle.samlevende_ved_indkomstårets_udløb = !cohabits;
    cases.push({ case_id: `conflicting-cohabitation-${cohabits}`, input: x, invalid: true });
  }
  const missing = input(0, 0);
  missing.helårsgrundlag.personskat.ægtefælle = v('UdenÆgtefælle');
  cases.push({ case_id: 'missing-annual-spouse', input: missing, invalid: true });
  const wrongYear = input(0, 0);
  wrongYear.personskat.ægtefælle.fakta.lønmodtager.skatteår = 2024;
  wrongYear.helårsgrundlag.personskat.ægtefælle.fakta.lønmodtager.skatteår = 2024;
  cases.push({ case_id: 'spouse-wrong-year', input: wrongYear, invalid: true });
  envelope.cases = cases.map(({ case_id, input }) => ({ case_id, input }));
  const output = run(['call', model, '--input', save(directory, 'input.json', envelope)]);
  save(directory, 'results.json', output);
  assert.deepEqual(output.diagnostics, []);
  assert.deepEqual(output.results.map(row => row.case_id), cases.map(row => row.case_id));
  for (const { case_id, result: r } of output.results) console.log(JSON.stringify({ case_id,
    valid: r.input_gyldigt, comparison: r.vurdering.slutskat_til_sammenligning_øre,
    faults: r.vurdering.fejl, shareBasis: r.delårsresultat.aktieindkomst_parår,
    shares: r.statslige_skattekomponenter.filter(c => ['Par14EndeligAktieindkomstskat', 'Par14Par8AStk2Skat'].includes(c.art.$variant)) }));
  for (const [i, c] of cases.entries()) await t.test(c.case_id, () => {
    const r = output.results[i].result, a = r.vurdering;
    assert.equal(r.input_gyldigt, !c.invalid, c.case_id);
    assert.equal(a.alle_kontroller_gyldige, !c.invalid);
    assert.equal(a.samlet_modeldækning_bekræftet, false);
    if (c.invalid) {
      assert.equal(a.slutskat_til_sammenligning_øre, null);
      assert.ok(a.fejl.some(f => f.sti === 'helårsgrundlag' && f.forklaring.includes('samliv ved indkomstårets udløb')));
      return;
    }
    assert.deepEqual(a.fejl, []);
    assert.equal(a.slutskat_til_sammenligning_øre, r.slutskat_efter_par14_øre);
    // Recorded ordinary-tax control + independently calculated share components,
    // not an independently observed administrative part-year total.
    assert.equal(a.slutskat_til_sammenligning_øre, c.total);
    assert.equal(r.kilder_afstemt_med_personskat, true);
    assert.equal(r.helårsgrundlag_gyldigt, true);
    const p = r.delårsresultat.aktieindkomst_parår;
    assert.equal(p.input.egen_aktieindkomst_kroner, c.input.kilder[1].delårsbeløb_kroner);
    assert.equal(p.egen_aktieindkomst_efter_ægtefællemodregning_kroner, c.net);
    for (const [art, expected] of [['Par14EndeligAktieindkomstskat', c.low], ['Par14Par8AStk2Skat', c.high]]) {
      const component = r.statslige_skattekomponenter.find(x => x.art.$variant === art);
      assert.equal(component.helårsskat_øre, expected, art);
      assert.equal(component.delårsskat_øre, expected, art);
      assert.equal(component.nedsættelsesfaktor_tæller_kroner, c.net, art);
      assert.equal(component.nedsættelsesfaktor_nævner_kroner, c.net, art);
    }
    const low = r.statslige_skattekomponenter.find(x => x.art.$variant === 'Par14EndeligAktieindkomstskat');
    assert.equal(low.helårsskat_øre, r.delårsresultat.endelig_aktieindkomstskat_øre);
  });
});

test('generated share guidance separates source facts, tax netting and year-end relationship facts', enabled, () => {
  const schema = run(['schema', model, '--format', 'compact-json']);
  const annual = schema.field_metadata.find(f => f.path === 'helårsgrundlag.$variant');
  assert.ok(annual?.help.includes('Samliv ved indkomstårets udløb skal være samme oplysning i begge grundlag'));
  const amount = schema.field_metadata.find(f => f.label === 'Beløb i skattepligtsperioden');
  assert.ok(amount?.help.includes('egen aktieindkomst før PSL § 8 a-ægtefællemodregning'));
  assert.ok(amount.help.includes('ikke det modregnede skattegrundlag'));
  const sources = schema.source_groups[amount.source_group].map(id => schema.source_objects[id]);
  assert.ok(sources.some(s => s.role === 'source' && JSON.stringify(s).includes('2021/1284')));
  assert.ok(sources.some(s => s.role === 'guidance' && JSON.stringify(s).includes('oid=1977389')));
});
