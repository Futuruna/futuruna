// Construction/presentation checks use fictional fixtures, not legal oracles.
// The opt-in integration invokes the canonical model; no compiler is built.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { buildCoupleCases, personFactFields, runCoupleDemo, summarizeCouple } from '../examples/danish-income-tax/pension-par-demo.mjs';
import { incomeFields, resultFields } from '../examples/danish-income-tax/personskat-resultat.mjs';
import { parseReportOutput } from '../examples/danish-income-tax/resultat-visning.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const hash = 'b'.repeat(64);
const v = ($variant, fields = {}) => ({ $variant, ...fields });
function template() {
  return { $futuruna: { schema_hash: hash }, cases: [{ case_id: 'template', input: {
    ...Object.fromEntries(personFactFields.map(key => [key, {}])),
    lønmodtager: {
      skatteår: 0, bruttoløn_kroner: 0, kommune: v('København'), kirkeskat: { $variant: 'KirkeskatUoplyst' },
      pension: { fødselsdato: { år: 0, måned: 0, dag: 0 }, atp: v('AtpUoplyst'), pbl18_indbetalinger: [],
        udbetalingsoplysninger: { for_året_komplette: false, for_foregående_år_komplette: false } },
      ligningsfradrag: { enlig_forsørger: v('EkstraBørnetilskudUoplyst'), boligjob: v('BoligjobUoplyst'),
        arbejdsfradrag_udland: v('ArbejdsfradragUdlandUoplyst') },
    },
    kapitalindkomst: { renter: { renteindtægter_kroner: 0, renteudgifter_kroner: 0 } },
    ægtefælle: v('UdenÆgtefælle'), negativ_aktieskat_fremførsel: {}, ligningslov33: {}, årsopgørelse: {},
  } }] };
}
function outputFixture() {
  return { $futuruna: { schema: 'futuruna.calculate.output.v1', schema_hash: hash, entry: 'beregn_personskat' },
    diagnostics: [], results: [['a-before', 400000], ['a-after', 400000], ['b-before', 20851073], ['b-after', 20467773]]
      .map(([case_id, tax]) => ({ case_id, result: {
        ...Object.fromEntries(resultFields.map(field => [field, {}])), slutskat_øre: tax,
        skat: { ...Object.fromEntries(incomeFields.map(([field]) => [field, 0])), skatteår: 2025 },
        ægtefælle: v('BeregnetÆgtefælle', { fakta: {}, grundlag: {}, skat: {}, samlevende_ved_indkomstårets_udløb: true }),
        årsopgørelse: v('IngenÅrsopgørelse'),
        vurdering: { status: v('BeregnetMedForbehold'), alle_kontroller_gyldige: true,
          slutskat_til_sammenligning_øre: tax, samlet_modeldækning_bekræftet: false,
          kontroller: [{ sti: 'fiktiv', gyldig: true, forklaring: 'Kun en visningsfixture.' }],
          fejl: [], forbehold: ['Fiktivt forbehold, som skal bevares.'] },
      } })) };
}
const summarize = output => summarizeCouple(parseReportOutput(JSON.stringify(output)), hash);
const facts = input => Object.fromEntries(personFactFields.map(key => [key, input[key]]));

test('couple construction mirrors all canonical person fields and changes only the private payment', () => {
  const source = readFileSync(join(root, 'examples/danish-income-tax/personskat.calculate.runa'), 'utf8');
  const fields = source.match(/^# PersonskatPersonFakta\((.+)\)$/m)?.[1];
  assert.deepEqual(fields.split(', ').map(f => f.split(':')[0]), [...personFactFields]);
  const t = template(), original = structuredClone(t);
  const { cases } = buildCoupleCases(t);
  assert.deepEqual(t, original);
  assert.deepEqual(cases.map(c => c.case_id), ['a-before', 'a-after', 'b-before', 'b-after']);
  const [a0, a1, b0, b1] = cases.map(c => c.input);
  for (const [a, b] of [[a0, b0], [a1, b1]]) {
    assert.deepEqual(a.ægtefælle.fakta, facts(b));
    assert.deepEqual(b.ægtefælle.fakta, facts(a));
    assert.equal(a.lønmodtager.bruttoløn_kroner, 50000);
    assert.equal(b.lønmodtager.bruttoløn_kroner, 600000);
  }
  assert.deepEqual(facts(b0), facts(b1));
  const aWithoutPayment = structuredClone(facts(a1));
  aWithoutPayment.lønmodtager.pension.pbl18_indbetalinger = [];
  assert.deepEqual(facts(a0), aWithoutPayment);
  const [payment] = a1.lønmodtager.pension.pbl18_indbetalinger;
  assert.equal(payment.betaling.beløb_kroner, 10000);
  assert.equal(payment.betaling.arbejdsmarkedsbidrag_kroner, 0);
  assert.equal(payment.indbetalingskilde.$variant, 'Pbl18EgenIndbetaling');
  for (const { input } of cases) {
    assert.equal(input.ægtefælle.samlevende_ved_indkomstårets_udløb, true);
    assert.deepEqual(input.ægtefælle.kildeskat25a_fordelinger, []);
    for (const key of ['negativ_aktieskat_fremførsel', 'ligningslov33', 'årsopgørelse']) {
      assert.deepEqual(input[key], original.cases[0].input[key]);
    }
    assert.equal(input.lønmodtager.pension.atp.$variant, 'IngenAtpIndbetalinger');
    assert.equal(input.lønmodtager.skatteår, 2025);
  }
  a1.lønmodtager.pension.pbl18_indbetalinger[0].betaling.beløb_kroner = 99;
  assert.equal(b1.ægtefælle.fakta.lønmodtager.pension.pbl18_indbetalinger[0].betaling.beløb_kroner, 10000,
    'separate before/after values, not shared mutable objects');
});

test('zero own saving does not hide a spouse saving or misstate household cash', () => {
  const output = outputFixture(), original = structuredClone(output);
  const { summary: s, text, details, exitCode } = summarize(output);
  assert.equal(exitCode, 0);
  assert.deepEqual([s.a_mindre_skat_øre, s.b_mindre_skat_øre, s.husstand_mindre_skat_øre,
    s.husstand_færre_frie_midler_øre], [0n, 383300n, 383300n, 616700n]);
  assert.equal(s.husstand_skat_før_øre, 21251073n);
  assert.equal(s.husstand_skat_efter_øre, 20867773n);
  assert.match(text, /A: mindre modelleret skat: 0,00 kr/);
  assert.match(text, /Husstanden: færre frie midler efter skat: 6\.167,00 kr/);
  assert.equal(s.cases.length, 4);
  assert.match(details, /Fiktivt forbehold, som skal bevares/);
  assert.deepEqual(output, original);
  output.results.reverse();
  assert.deepEqual(summarize(output).summary, s, 'order does not change case identity');
});

test('each invalid, missing or diagnostic case withholds the entire household comparison', () => {
  for (const index of [0, 1, 2, 3]) {
    const output = outputFixture(), a = output.results[index].result.vurdering;
    a.status = v('UgyldigtBeregningsgrundlag'); a.alle_kontroller_gyldige = false;
    a.slutskat_til_sammenligning_øre = null;
    a.kontroller[0].gyldig = false; a.fejl = structuredClone(a.kontroller);
    const r = summarize(output);
    assert.equal(r.exitCode, 2);
    for (const key of ['a_mindre_skat_øre', 'b_mindre_skat_øre', 'husstand_mindre_skat_øre',
      'husstand_færre_frie_midler_øre', 'husstand_skat_før_øre', 'husstand_skat_efter_øre']) {
      assert.equal(r.summary[key], null, `${index}: ${key}`);
    }
    assert.match(r.text, /Ingen husstandssammenligning/);
    assert.match(r.details, /Kun en visningsfixture/);
  }
  const output = outputFixture(); output.results.pop();
  assert.equal(summarize(output).exitCode, 2, 'missing row alone is not a zero tax');
  output.diagnostics.push({ case_id: 'b-after', path: '$.input', message: 'Fiktiv afvist sag.' });
  const result = summarize(output);
  assert.equal(result.exitCode, 2); assert.match(result.details, /Fiktiv afvist sag/);
  assert.deepEqual(result.summary.diagnostics, parseReportOutput(JSON.stringify(output)).diagnostics);
});

test('mismatched contracts, contradictory gates and other profiles cannot become a comparison', () => {
  for (const edit of [
    o => { o.$futuruna.schema_hash = 'c'.repeat(64); },
    o => { o.results[0].case_id = 'another-person'; },
    o => { o.results[0].result.skat.skatteår = 2026; },
    o => { o.results[0].result.ægtefælle = v('IngenÆgtefælleberegning'); },
    o => { o.results[0].result.ægtefælle.samlevende_ved_indkomstårets_udløb = false; },
    o => { o.results[0].result.vurdering.slutskat_til_sammenligning_øre = null; },
    o => { o.results[0].result.vurdering.forbehold = []; },
    o => { o.results.push(structuredClone(o.results[0])); },
  ]) {
    const output = outputFixture(); edit(output); assert.throws(() => summarize(output));
  }
});

test('public couple CLI uses one worker and retains inputs, exact output and all caveats', () => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-couple-cli-test-'));
  const binary = join(directory, 'fixture-runa');
  // Stub tests orchestration only. It cannot certify any tax result.
  writeFileSync(binary, `#!${process.execPath}
const args = process.argv.slice(2);
if (process.env.FUTURUNA_CALCULATION_JOBS !== '1') process.exit(40);
if (args[0] === 'template') console.log(${JSON.stringify(JSON.stringify(template()))});
else if (args[0] === 'call') console.log(${JSON.stringify(JSON.stringify(outputFixture()))});
else process.exit(41);
`, { flag: 'wx', mode: 0o700 });
  const p = spawnSync(process.execPath, ['examples/danish-income-tax/pension-par-demo.mjs', binary], {
    cwd: root, encoding: 'utf8', timeout: 10000,
  });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr);
  const evidence = p.stderr.match(/^Fiktiv dokumentation: (.+)$/m)?.[1];
  assert.ok(evidence);
  assert.deepEqual(JSON.parse(readFileSync(join(evidence, 'cases.json'), 'utf8')), buildCoupleCases(template()));
  assert.deepEqual(JSON.parse(readFileSync(join(evidence, 'results.json'), 'utf8')), outputFixture());
  assert.equal(JSON.parse(readFileSync(join(evidence, 'summary.json'), 'utf8')).husstand_mindre_skat_øre, 383300);
  assert.match(readFileSync(join(evidence, 'resultat.txt'), 'utf8'), /Fiktivt forbehold, som skal bevares/);
  assert.match(p.stdout, /Husstanden: færre frie midler efter skat: 6\.167,00 kr/);
});

test('canonical four-case output keeps own and household pension effects separate', {
  skip: !process.env.FUTURUNA_MODEL_TEST_RUNA && 'Set FUTURUNA_MODEL_TEST_RUNA; four canonical cases, no build or network.',
}, () => {
  const r = runCoupleDemo(process.env.FUTURUNA_MODEL_TEST_RUNA);
  assert.equal(r.exitCode, 0);
  assert.deepEqual(r.summary.cases.map(row => row.vurdering.slutskat_til_sammenligning_øre),
    [400000n, 400000n, 20851073n, 20467773n], 'recorded model amounts in the guide, not an external oracle');
  assert.deepEqual([r.summary.a_mindre_skat_øre, r.summary.b_mindre_skat_øre,
    r.summary.husstand_mindre_skat_øre, r.summary.husstand_færre_frie_midler_øre], [0n, 383300n, 383300n, 616700n]);
  for (const row of r.summary.cases) {
    assert.equal(row.vurdering.alle_kontroller_gyldige, true);
    assert.equal(row.vurdering.samlet_modeldækning_bekræftet, false);
  }
});
