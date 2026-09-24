// Fictional, reproducible interview example. All tax rules run in Futuruna.
// This is NOT an importer or a template for someone's real tax facts.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const args = process.argv.slice(2);
if (args.length !== 1 || args[0] === '--help') {
  console.log('Usage: node examples/danish-income-tax/pension-demo.mjs PATH_TO_RUNA');
  console.log('Runs eight fictional 2026 cases serially; preserves evidence in a new temporary directory.');
  process.exit(args[0] === '--help' ? 0 : 1);
}
const binary = args[0].includes('/') || args[0].includes('\\') ? resolve(args[0]) : args[0];
const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const model = 'examples/danish-income-tax/personskat.calculate.runa';
const evidence = mkdtempSync(join(tmpdir(), 'futuruna-pension-demo-'));
console.error(`Fictional example only. Evidence: ${evidence}`);

function run(arguments_) {
  const process_ = spawnSync(binary, arguments_, {
    cwd: root, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024,
    env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' },
  });
  if (process_.stderr) process.stderr.write(process_.stderr);
  if (process_.error) throw process_.error;
  assert.equal(process_.status, 0, `runa ${arguments_.join(' ')} failed:\n${process_.stdout}`);
  return JSON.parse(process_.stdout);
}
function save(name, value) {
  const path = join(evidence, name);
  writeFileSync(path, JSON.stringify(value, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
  return path;
}
const variant = ($variant) => ({ $variant });
function payment(id, amount, employer = false) {
  return {
    identifikation: id,
    ordning: variant('Pbl18Rateopsparing'),
    indbetalingskilde: variant(employer ? 'Pbl18Arbejdsgiverindbetaling' : 'Pbl18EgenIndbetaling'),
    fradragsretshaver: variant('Pbl18OrdningensEjer'),
    betaling: {
      beløb_kroner: amount, forfaldsår: 2026, betalingsår: 2026,
      betalt_senest_bankjusteret_1_april_efter_forfald: true,
      hidrører_fra_par22e_tilbagebetaling: false,
      par15a_fradragsplacering: variant('Pbl18IkkePar15APlacering'),
      arbejdsmarkedsbidrag_kroner: employer ? 4000 : 0,
    },
    fordelingsforløb: variant('Pbl18IngenTiårsfordeling'),
    indeksvalg: { fradragsvalgte_kontraktbidrag_kroner: [] },
    indeksordningsgrundlag: variant('Pbl18IkkeIndeksordning'),
    forfaldne_ikke_tidligere_fratrukket_kroner: 0,
    særligt_ordningsgrundlag: variant('Pbl18IntetSærligtOrdningsgrundlag'),
    begrænsninger: {
      pbl54_personkreds_opfyldt: true,
      afgiftspligt_for_hele_ordningen_indtrådt: false,
      udenlandsk_overførsel_med_tidligere_fradrag_uden_skatte_eller_afgiftskonsekvens: false,
    },
  };
}

const envelope = run(['template', model, '--format', 'json']);
const baseline = envelope.cases[0].input;
// Declared fictional facts: resident adult employee, Copenhagen, no church tax,
// spouse, property, other income, losses, pension payouts or other deductions.
// Empty/default branches describe this invented person ONLY, never missing data.
Object.assign(baseline.lønmodtager, {
  skatteår: 2026, bruttoløn_kroner: 600000,
  kommune: variant('København'), betaler_kirkeskat: false,
});
baseline.lønmodtager.pension.fødselsdato = { år: 1990, måned: 1, dag: 1 };
baseline.lønmodtager.pension.atp = { $variant: "IngenAtpIndbetalinger" };
baseline.lønmodtager.pension.udbetalingsoplysninger = {
  for_året_komplette: true, for_foregående_år_komplette: true,
}; // Explicitly confirmed absence in this fictional person only.
baseline.lønmodtager.ligningsfradrag.enlig_forsørger = variant('IntetEkstraBørnetilskud');
baseline.lønmodtager.ligningsfradrag.boligjob = variant('IngenBoligjobudgifter');
// Declared fictional treaty-residence fact, not inferred from Copenhagen.
// One false condition excludes the foreign-income exception; the other two
// may remain unknown. Never copy this assertion into a real person's case.
baseline.lønmodtager.ligningsfradrag.arbejdsfradrag_udland = {
  $variant: 'IngenUdlandsudelukkelseIFællesForhold',
  dbo_hjemmehørende_udland_i_nogen_periode: false,
  noget_arbejde_udført_udland: null,
  nogen_udenlandsk_arbejdsgiver: null,
  kildereference: 'Fiktiv person: DBO-hjemmehørende i Danmark hele året',
};
assert.deepEqual(baseline.ægtefælle, variant('UdenÆgtefælle'));

const specifications = [
  ['uden-pension', 0, false],
  ['privat-30000', 30000, false],
  ['privat-40000', 40000, false],
  ['privat-50000', 50000, false],
  ['med-arbejdsgiver-20000', 20000, true],
  ['med-arbejdsgiver-22700', 22700, true],
  ['med-arbejdsgiver-22701', 22701, true],
  ['uoplyste-fradrag', 40000, false],
];
envelope.cases = specifications.map(([case_id, amount, employer]) => {
  const input = structuredClone(baseline);
  const contributions = input.lønmodtager.pension.pbl18_indbetalinger;
  if (amount > 0) contributions.push(payment('fiktiv-privat-rate', amount));
  // Separate fixed compensation scenario: 50,000 gross, 4,000 documented AM.
  // Not a salary-sacrifice comparison with the private-only cases.
  if (employer) contributions.push(payment('fiktiv-arbejdsgiver-rate', 50000, true));
  if (case_id === 'uoplyste-fradrag') {
    input.lønmodtager.ligningsfradrag.boligjob = variant('BoligjobUoplyst');
  }
  return { case_id, input };
});
const inputPath = save('cases.json', envelope);
console.error('Calculating eight cases with one worker…');
const output = run(['call', model, '--input', inputPath]);
save('results.json', output);
assert.deepEqual(output.diagnostics, []);
assert.equal(output.results.length, specifications.length);

const rows = output.results.map(({ case_id, result }, index) => {
  assert.equal(case_id, specifications[index][0]);
  const gate = result.vurdering;
  const invalid = case_id === 'uoplyste-fradrag';
  assert.equal(gate.status.$variant, invalid ? 'UgyldigtBeregningsgrundlag' : 'BeregnetMedForbehold');
  assert.equal(gate.alle_kontroller_gyldige, !invalid);
  assert.equal(gate.samlet_modeldækning_bekræftet, false);
  assert.ok(gate.forbehold.length > 0);
  const tax = gate.slutskat_til_sammenligning_øre;
  if (invalid) {
    assert.equal(tax, null);
    assert.ok(gate.fejl.some((error) => error.sti === 'lønmodtager.ligningsfradrag.boligjob'));
  } else {
    assert.ok(Number.isSafeInteger(tax));
    assert.deepEqual(gate.fejl, []);
  }
  const pension = result.pension.pbl18_årsresultat;
  return {
    case_id, privat_indbetaling_kroner: specifications[index][1],
    arbejdsgiver_brutto_kroner: specifications[index][2] ? 50000 : 0,
    arbejdsgiver_am_kroner: specifications[index][2] ? 4000 : 0,
    privat_fradragsloft_kroner: invalid ? null : pension.rate_og_ophørende_fradragsloft_efter_arbejdsgiver_kroner,
    privat_ratefradrag_kroner: invalid ? null : pension.rate_og_ophørende_fradrag_kroner,
    ikke_fratrukket_kroner: invalid ? null : pension.egne_indbetalinger_ikke_fratrukket_i_indkomståret_kroner,
    ekstra_pensionsfradrag_kroner: invalid ? null : result.skat.ekstra_pensionsfradrag_kroner,
    slutskat_øre: tax, vurdering: gate,
  };
});
const byId = Object.fromEntries(rows.map((row) => [row.case_id, row]));
assert.equal(byId['uden-pension'].slutskat_øre, 20872564);
for (const amount of [30000, 40000, 50000]) {
  const row = byId[`privat-${amount}`];
  assert.equal(row.privat_ratefradrag_kroner, amount);
  assert.equal(row.privat_fradragsloft_kroner, 68700);
  assert.equal(row.ekstra_pensionsfradrag_kroner, amount * 12 / 100);
}
for (const amount of [20000, 22700, 22701]) {
  const row = byId[`med-arbejdsgiver-${amount}`];
  assert.equal(row.privat_fradragsloft_kroner, 22700);
  assert.equal(row.privat_ratefradrag_kroner, Math.min(amount, 22700));
  assert.equal(row.ikke_fratrukket_kroner, Math.max(0, amount - 22700));
}
assert.equal(byId['med-arbejdsgiver-22700'].slutskat_øre, byId['med-arbejdsgiver-22701'].slutskat_øre);

function compare(beforeId, afterId) {
  const before = byId[beforeId];
  const after = byId[afterId];
  // Only compare like-for-like compensation, and never subtract null as zero.
  assert.equal(before.arbejdsgiver_brutto_kroner, after.arbejdsgiver_brutto_kroner);
  const permitted = before.slutskat_øre !== null && after.slutskat_øre !== null;
  const contributionChange = (after.privat_indbetaling_kroner - before.privat_indbetaling_kroner) * 100;
  const saving = permitted ? before.slutskat_øre - after.slutskat_øre : null;
  return {
    før: beforeId, efter: afterId, indbetalingsændring_øre: contributionChange,
    mindre_modelleret_skat_øre: saving,
    mindre_frie_midler_efter_skat_øre: permitted ? contributionChange - saving : null,
  };
}
const summary = {
  fiktivt_eksempel: true, skatteår: 2026,
  forklaring: 'Positive ændringer betyder mere indbetalt, mindre skat og færre frie midler. Ikke en vurdering af livsvarigt afkast, ydelser eller skatten ved udbetaling.',
  kontrakt: envelope.$futuruna,
  sammenligninger: [
    compare('privat-40000', 'privat-30000'),
    compare('privat-40000', 'privat-50000'),
    compare('med-arbejdsgiver-20000', 'med-arbejdsgiver-22700'),
    compare('med-arbejdsgiver-22700', 'med-arbejdsgiver-22701'),
    compare('privat-40000', 'uoplyste-fradrag'),
  ],
  cases: rows,
};
assert.deepEqual(summary.sammenligninger.map((row) => [
  row.indbetalingsændring_øre,
  row.mindre_modelleret_skat_øre,
  row.mindre_frie_midler_efter_skat_øre,
]), [
  [-1000000, -382068, -617932],
  [1000000, 382068, 617932],
  [270000, 103159, 166841],
  [100, 0, 100],
  [0, null, null],
]);
save('summary.json', summary);
const kroner = new Intl.NumberFormat('da-DK', { style: 'currency', currency: 'DKK' });
const amount = (ore) => kroner.format(Math.abs(ore) / 100);
console.log('Fiktivt pensionseksempel, 2026. Beregnet med forbehold — ikke personlig rådgivning.');
for (const row of summary.sammenligninger) {
  const label = `${row.før} → ${row.efter}`;
  if (row.mindre_modelleret_skat_øre === null) {
    console.log(`${label}: Ingen sammenligning — nødvendige fradragsoplysninger er uafklarede.`);
    continue;
  }
  const contribution = row.indbetalingsændring_øre;
  const saving = row.mindre_modelleret_skat_øre;
  const cash = row.mindre_frie_midler_efter_skat_øre;
  const taxChange = saving === 0 ? 'uændret skat' : `${amount(saving)} ${saving > 0 ? 'mindre' : 'mere'} modelleret skat`;
  console.log(`${label}: ${amount(contribution)} ${contribution >= 0 ? 'mere' : 'mindre'} indbetalt; ${taxChange}; ${amount(cash)} ${cash >= 0 ? 'færre' : 'flere'} frie midler.`);
}
console.log('Skatteændring er ikke nødvendigvis ændret tilbagebetaling. Fremtidig pensionsskat, afkast, omkostninger og ydelser er ikke medregnet.');
console.log(`Otte fiktive sager kontrolleret. Input, resultater, summary.json og forbehold: ${evidence}`);
