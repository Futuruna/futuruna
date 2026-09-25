// One explicitly fictional couple, not a generic spouse-swapping adapter.
// Futuruna computes all tax; this script only constructs cases and sums/diffs
// the four independently gated, main-person comparison amounts.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildFictionalCases } from './bilag-demo.mjs';
import { renderPersonskatOutput } from './personskat-resultat.mjs';
import { amount, readSavedOutput, visible } from './resultat-visning.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const model = 'examples/danish-income-tax/personskat.calculate.runa';
const caseIds = ['a-before', 'a-after', 'b-before', 'b-after'];
export const personFactFields = Object.freeze(['lønmodtager', 'kapitalindkomst', 'aktieavance',
  'udenlandske_sociale_bidrag', 'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter']);

export function buildCoupleCases(template) {
  // Reuse only fictional construction, NOT the single-person source ledger or
  // its independent expected taxes. The couple's facts are stated in the guide.
  const base = buildFictionalCases(template).envelope.cases.find(c => c.case_id === 'privat-rate').input;
  const facts = () => Object.fromEntries(personFactFields.map(key => {
    assert.ok(Object.hasOwn(base, key), `Skabelonen mangler ${key}.`);
    return [key, structuredClone(base[key])];
  }));
  const aBefore = facts(), aAfter = facts(), b = facts();
  aBefore.lønmodtager.bruttoløn_kroner = 50000;
  aBefore.lønmodtager.pension.pbl18_indbetalinger = [];
  aAfter.lønmodtager.bruttoløn_kroner = 50000;
  aAfter.lønmodtager.pension.pbl18_indbetalinger[0].betaling.beløb_kroner = 10000;
  b.lønmodtager.pension.pbl18_indbetalinger = [];
  const make = (case_id, own, spouse) => ({ case_id, input: {
    ...structuredClone(base), ...structuredClone(own),
    ægtefælle: { $variant: 'MedÆgtefælle', fakta: structuredClone(spouse),
      samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] },
  } });
  // Pair-level losses, foreign relief and settlement remain neutral in this
  // fixed example. Arbitrary personal cases cannot safely use this swap.
  return { ...structuredClone(template), cases: [make(caseIds[0], aBefore, b),
    make(caseIds[1], aAfter, b), make(caseIds[2], b, aBefore), make(caseIds[3], b, aAfter)] };
}

export function summarizeCouple(output, schemaHash) {
  // The existing viewer checks exact units, gate consistency, all warnings and
  // diagnostic structure. It never substitutes diagnostic tax for withheld tax.
  const rendered = renderPersonskatOutput(output);
  assert.equal(output.$futuruna.schema_hash, schemaHash, 'Input og output har forskellig kontrakt.');
  const byId = new Map(output.results.map(row => [row.case_id, row.result]));
  for (const row of [...output.results, ...output.diagnostics]) {
    assert.ok(caseIds.includes(row.case_id), 'Resultatet tilhører ikke det faste pareksempel.');
  }
  for (const result of byId.values()) {
    assert.equal(result.skat.skatteår, 2025n);
    assert.equal(result.ægtefælle.$variant, 'BeregnetÆgtefælle');
    assert.equal(result.ægtefælle.samlevende_ved_indkomstårets_udløb, true);
  }
  const permitted = rendered.exitCode === 0 && caseIds.every(id => byId.has(id));
  const tax = id => byId.get(id).vurdering.slutskat_til_sammenligning_øre;
  const ownSaving = permitted ? tax('a-before') - tax('a-after') : null;
  const spouseSaving = permitted ? tax('b-before') - tax('b-after') : null;
  const householdSaving = permitted ? ownSaving + spouseSaving : null;
  const summary = {
    fiktivt_eksempel: true, skatteår: 2025, kontrakt: output.$futuruna,
    status: permitted ? 'SammenlignetMedForbehold' : 'SammenligningTilbageholdt',
    indbetalingsændring_øre: 1000000n,
    a_mindre_skat_øre: ownSaving, b_mindre_skat_øre: spouseSaving,
    husstand_skat_før_øre: permitted ? tax('a-before') + tax('b-before') : null,
    husstand_skat_efter_øre: permitted ? tax('a-after') + tax('b-after') : null,
    husstand_mindre_skat_øre: householdSaving,
    husstand_færre_frie_midler_øre: permitted ? 1000000n - householdSaving : null,
    forklaring: 'Kun dette fiktive par med uændret løn og øvrige betalinger. Positive forskelle betyder mindre skat og færre frie midler. Ikke tilbagebetaling, samlet pensionsøkonomi eller uafhængig SKAT-konformitet.',
    cases: caseIds.map(case_id => ({ case_id, vurdering: byId.get(case_id)?.vurdering ?? null })),
    diagnostics: output.diagnostics,
  };
  const text = ['Fiktivt ægtepar, 2025 — modelresultat med forbehold, ikke personlig rådgivning.',
    'A indbetaler 10.000 kr. mere privat; begge lønninger og øvrige betalinger er uændrede.'];
  if (permitted) {
    text.push(`A: mindre modelleret skat: ${amount(ownSaving, 'øre')}`,
      `B: mindre modelleret skat: ${amount(spouseSaving, 'øre')}`,
      `Husstanden: mindre modelleret skat: ${amount(householdSaving, 'øre')}`,
      `Husstanden: færre frie midler efter skat: ${amount(summary.husstand_færre_frie_midler_øre, 'øre')}`);
  } else text.push('Ingen husstandssammenligning: alle fire gyldige sammenligningsbeløb er nødvendige. Ukendt er ikke nul.');
  text.push(summary.forklaring, 'Alle fire vurderinger, forbehold og eventuelle fejl skal læses i resultat.txt og results.json.');
  return { summary, text: text.join('\n') + '\n', details: rendered.text, exitCode: permitted ? 0 : 2 };
}

function save(directory, name, value) {
  const text = typeof value === 'string' ? value : JSON.stringify(value, (_key, v) => {
    if (typeof v !== 'bigint') return v;
    const number = Number(v);
    assert.ok(Number.isSafeInteger(number), 'Eksemplets JSON-oversigt kræver sikre heltalsbeløb.');
    return number;
  }, 2) + '\n';
  const path = join(directory, name);
  writeFileSync(path, text, { flag: 'wx', mode: 0o600 });
  return path;
}

// Display can be checked against preserved, unchanged calculation evidence.
// This helper does not authenticate saved output or revalidate source facts.
export function finishCoupleDemo(evidence, schemaHash) {
  const result = summarizeCouple(readSavedOutput(join(evidence, 'results.json')), schemaHash);
  save(evidence, 'summary.json', result.summary);
  save(evidence, 'resultat.txt', result.details);
  save(evidence, 'sammenligning.txt', result.text);
  console.log(result.text + `Fiktive input, resultater og forbehold: ${visible(evidence)}`);
  return result;
}

export function runCoupleDemo(binary) {
  binary = binary.includes('/') || binary.includes('\\') ? resolve(binary) : binary;
  const evidence = mkdtempSync(join(tmpdir(), 'futuruna-pension-par-'));
  console.error(`Fiktiv dokumentation: ${visible(evidence)}`);
  const run = (args, acceptDiagnostics = false) => {
    const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 600000,
      maxBuffer: 24 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
    assert.ifError(p.error);
    assert.ok(p.status === 0 || (acceptDiagnostics && p.status === 1), p.stderr);
    return p.stdout;
  };
  const template = JSON.parse(run(['template', model, '--entry', 'beregn_personskat', '--format', 'json']));
  const envelope = buildCoupleCases(template);
  const input = save(evidence, 'cases.json', envelope);
  console.error('Beregner fire fiktive sager med én worker; ingen anden skatteberegner.');
  save(evidence, 'results.json', run(['call', model, '--entry', 'beregn_personskat', '--input', input], true));
  return { evidence, ...finishCoupleDemo(evidence, envelope.$futuruna.schema_hash) };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  if (args.length !== 1 || args[0] === '--help') {
    console.log('Usage: node examples/danish-income-tax/pension-par-demo.mjs PATH_TO_VERIFIED_RUNA');
    console.log('Kun ét fast fiktivt ægtepar; ingen import af personlige data, installation eller netværk.');
    process.exitCode = args[0] === '--help' ? 0 : 1;
  } else {
    try { process.exitCode = runCoupleDemo(args[0]).exitCode; }
    catch (error) { console.error(`Eksemplet kunne ikke afsluttes: ${visible(error.message)}`); process.exitCode = 1; }
  }
}
