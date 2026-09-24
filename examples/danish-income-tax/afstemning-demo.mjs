// Fictional report observations only. Futuruna, not this script, evaluates
// the reconciliation. Never use these invented observations as personal facts.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const args = process.argv.slice(2);
if (args.length !== 1 || args[0] === '--help') {
  console.log('Usage: node examples/danish-income-tax/afstemning-demo.mjs PATH_TO_RUNA');
  console.log('Fire fiktive rapporter, ingen personlige dokumenter. Brug den compiler, der bestod kompatibilitetstjekket.');
  process.exit(args.length === 1 && args[0] === '--help' ? 0 : 1);
}
const binary = args[0].includes('/') || args[0].includes('\\') ? resolve(args[0]) : args[0];
const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const model = 'examples/danish-income-tax/aarsopgoerelse-afstemning.calculate.runa';
const evidence = mkdtempSync(join(tmpdir(), 'futuruna-afstemning-demo-'));
console.error(`Kun fiktive observationer. Input og resultater gemmes i: ${evidence}`);

function run(arguments_) {
  const child = spawnSync(binary, arguments_, {
    cwd: root, encoding: 'utf8', maxBuffer: 8 * 1024 * 1024,
    env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' },
  });
  if (child.stderr) process.stderr.write(child.stderr);
  if (child.error) throw child.error;
  assert.equal(child.status, 0, `runa failed:\n${child.stdout}`);
  return JSON.parse(child.stdout);
}
function save(name, value) {
  const path = join(evidence, name);
  writeFileSync(path, JSON.stringify(value, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
  return path;
}

// All observations are explicitly invented; no generated template defaults
// are adopted as facts. Unknown spouse municipality/cohabitation remain null.
const baseline = {
  skatteår: 2026, kommune: { $variant: 'København' }, betaler_kirkeskat: false,
  ægtefællens_kommune: null, gift_samlevende_ved_årets_udløb: null,
  indkomstbro: {
    personlig_indkomst_kroner: 400000, kapitalindkomst_kroner: -10000,
    ligningsmæssige_fradrag_kroner: 45000, skattepligtig_indkomst_kroner: 333000,
    øvrige_indkomstreguleringer_kroner: 0, oplyst_ægtefælleunderskud_kroner: 12000,
  },
  ægtefællenedslag: {
    statsligt_personfradrag_øre: 100000, kommunalt_personfradrag_øre: 200000,
    kirkeligt_personfradrag_øre: 0, negativ_kapitalindkomst_øre: 80000,
    eget_negativ_kapitalindkomstnedslag_øre: 80000,
  },
  skat: {
    poster_uden_ægtefællenedslag: [{ navn: 'Fiktiv øvrig skat', beløb_øre: 10200000 }],
    poster_uden_ægtefællenedslag_komplette: true, oplyst_beregnet_skat_øre: 9820000,
  },
  betaling: {
    oplyst_forskudsskat_øre: 9900000, oplyst_beregnet_skat_øre: 9820000,
    oplyst_overskydende_skat_øre: 80000,
    korrektioner_til_udbetaling: [{ navn: 'Fiktiv oplyst godtgørelse', beløb_øre: 1234 }],
    korrektioner_komplette: true, oplyst_udbetaling_kroner: 812, restskat: null,
  },
};
const missing = structuredClone(baseline);
missing.indkomstbro.oplyst_ægtefælleunderskud_kroner = null;
for (const field of ['statsligt_personfradrag_øre', 'kommunalt_personfradrag_øre',
  'kirkeligt_personfradrag_øre', 'negativ_kapitalindkomst_øre']) {
  missing.ægtefællenedslag[field] = null;
}
const conflict = structuredClone(baseline);
conflict.ægtefællenedslag.statsligt_personfradrag_øre = 100001;
const partialConflict = structuredClone(baseline);
partialConflict.ægtefællenedslag.kommunalt_personfradrag_øre = null;
partialConflict.skat.poster_uden_ægtefællenedslag[0].beløb_øre = 9999999;
const envelope = run(['template', model, '--format', 'json']);
envelope.cases = [
  { case_id: 'betinget-match', input: baseline },
  { case_id: 'manglende-overførselslinjer', input: missing },
  { case_id: 'en-øre-forskel', input: conflict },
  { case_id: 'modstrid-med-manglende-linje', input: partialConflict },
];
const inputPath = save('cases.json', envelope);
const output = run(['call', model, '--input', inputPath]);
save('results.json', output);
assert.deepEqual(output.diagnostics, []);
assert.deepEqual(output.results.map(({ case_id }) => case_id), envelope.cases.map(({ case_id }) => case_id));
assert.deepEqual(output.results.map(({ result }) => result.status.$variant),
  ['BetingetAfstemt', 'Ufuldstændig', 'Modstrid', 'Modstrid']);

function named(rows, name) {
  const matches = rows.filter((row) => row.navn === name);
  assert.equal(matches.length, 1, `Expected one output named ${name}`);
  return matches[0];
}
const lossName = 'Nødvendigt indkomstfradrag fra ægtefælle';
const creditName = 'Nødvendig samlet skattenedsættelse fra ægtefælle';
const creditControl = 'Samlet ægtefællenedslag i beregnet skat';
const residualName = 'Nødvendig sum af ikke-oplyste ægtefællenedslag';
const missingResult = output.results[1].result;
assert.equal(named(missingResult.kontroller, 'Indkomstbro og ægtefælleunderskud').oplyst, null);
assert.equal(named(missingResult.kontroller, creditControl).oplyst, null);
assert.equal(named(missingResult.kontroller, creditControl).difference, null);
assert.equal(named(output.results[2].result.kontroller, creditControl).difference, 1);
assert.equal(named(output.results[0].result.nødvendige_forudsætninger,
  'Ubrugt kommunal personfradragsværdi fra ægtefælle').højst, 1265399);
const partialResult = output.results[3].result;
const partialResidual = named(partialResult.nødvendige_forudsætninger, residualName);
assert.equal(partialResidual.nødvendigt_beløb, -1);
assert.equal(partialResidual.inden_for_kontrollerede_grænser, false);
assert.equal(partialResidual.højst, 1265399);
assert.equal(named(partialResult.kontroller, creditControl).oplyst, null);
assert.equal(named(partialResult.kontroller, creditControl).difference, null);

// Show exact model units: no floating-point currency conversion or tax logic.
const number = new Intl.NumberFormat('da-DK', { maximumFractionDigits: 0 });
function amount(value, unit) {
  if (value === null) return 'ukendt';
  assert.ok(Number.isSafeInteger(value));
  return `${number.format(value)} ${unit}`;
}
console.log('FIKTIV BETINGET AFSTEMNING — ikke uafhængig skatteberegning eller skatterådgivning.');
console.log('Beløbene er nødvendige betingelser, ikke dokumentation for ægtefællens faktiske forhold.');
console.log('Ægtefællens kommune og samlivsbetingelse er ukendte i alle fire eksempler.');
for (const { case_id, result } of output.results) {
  assert.equal(result.uafhængig_skatteberegning_udført, false);
  assert.ok(result.uafklaret.length > 0);
  const loss = named(result.nødvendige_forudsætninger, lossName);
  const credit = named(result.nødvendige_forudsætninger, creditName);
  assert.equal(loss.nødvendigt_beløb, 12000);
  assert.equal(loss.enhed, 'DKK');
  assert.equal(credit.nødvendigt_beløb, case_id === 'modstrid-med-manglende-linje' ? 179999 : 380000);
  assert.equal(credit.enhed, 'øre');
  console.log(`\n${case_id}: ${result.status.$variant}`);
  for (const condition of [loss, credit]) {
    console.log(`  ${condition.navn}: ${amount(condition.nødvendigt_beløb, condition.enhed)}`);
  }
  for (const condition of result.nødvendige_forudsætninger.filter((row) => row.navn === residualName)) {
    console.log(`  ${condition.navn}: ${amount(condition.nødvendigt_beløb, condition.enhed)}`);
    console.log(`    Mindst: ${amount(condition.mindst, condition.enhed)}; højst: ${amount(condition.højst, condition.enhed)}; inden for kontrollerede grænser: ${condition.inden_for_kontrollerede_grænser ? 'ja' : 'nej'}`);
    console.log(`    ${condition.forklaring}`);
  }
  for (const check of result.kontroller.filter((row) => row.status.$variant !== 'Stemmer')) {
    console.log(`  ${check.navn}: ${check.status.$variant}`);
    console.log(`    Forventet: ${amount(check.forventet, check.enhed)}; oplyst: ${amount(check.oplyst, check.enhed)}; difference (oplyst − forventet): ${amount(check.difference, check.enhed)}`);
  }
  console.log(`  ${result.uafklaret.length} forbehold bevares i results.json; status er ikke en godkendelse af skatteforholdene.`);
}
console.log(`\nFire fiktive cases kontrolleret. Fulde input, kontroller, grænser og forbehold: ${evidence}`);
