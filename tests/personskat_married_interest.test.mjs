// Independent anonymous SKAT 2025 observations, collected 2026-09-25.
// Fictional facts only; no network, personal documents or compiler build.
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
const enabled = { skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no automatic build.' };
const model = 'examples/danish-income-tax/personskat.calculate.runa';
function run(args) {
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 1200000,
    maxBuffer: 64 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
}
function save(directory, name, value) {
  const file = join(directory, name);
  writeFileSync(file, JSON.stringify(value), { flag: 'wx', mode: 0o600 }); return file;
}

// Source amounts are DKK. Each expected tuple is an independently observed
// [bundskat before allowances, municipal tax before allowances, applied PSL11,
// calculated tax INCLUDING AM but BEFORE settlement additions], all in øre.
// Applied PSL11 includes an incoming unused spouse credit where present.
const households = [
  { id: 'pooled-boundary', wages: [600000, 200000], expenses: [100001, 0], interest: [0, 0],
    observed: [[6629520, 9247226, 800000, 18044430], [2209840, 3745900, 0, 5723424]] },
  { id: 'split-negative', wages: [600000, 200000], expenses: [60000, 40000], interest: [0, 0],
    observed: [[6629520, 10187250, 480000, 19304454], [2209840, 2805900, 320000, 4463424]] },
  { id: 'positive-offset', wages: [600000, 200000], expenses: [100001, 0], interest: [0, 20000],
    observed: [[6629520, 9247226, 640008, 18204422], [2209840, 4215900, 0, 6193424]] },
  { id: 'unused-spouse-credit', wages: [600000, 0], expenses: [0, 40000], interest: [0, 0],
    observed: [[6629520, 10657250, 320000, 18102138], [0, 0, 0, 0]] },
];

test('both spouses match observed ordinary interest pooling, offsets and unused-credit transfers', enabled, () => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-married-interest-canonical-'));
  console.log(`Fictional married-interest evidence: ${directory}`);
  const envelope = run(['template', model, '--format', 'json']);
  // Reuse construction only, NOT the demo's single-person source ledger or
  // expectations. Each spouse is born 1990-01-01, resident in Copenhagen for
  // all of 2025, without church tax, children, pension, ATP or other amounts.
  const base = buildFictionalCases(envelope).envelope.cases[0].input;
  base.lønmodtager.pension.pbl18_indbetalinger = [];
  const expected = new Map();
  envelope.cases = households.flatMap(household => {
    const people = [0, 1].map(person => {
      const facts = Object.fromEntries(personFactFields.map(key => [key, structuredClone(base[key])]));
      facts.lønmodtager.bruttoløn_kroner = household.wages[person];
      facts.kapitalindkomst.renter.renteudgifter_kroner = household.expenses[person];
      facts.kapitalindkomst.renter.renteindtægter_kroner = household.interest[person];
      return facts;
    });
    return people.map((own, person) => {
      const case_id = `${household.id}-${person === 0 ? 'a' : 'b'}`;
      expected.set(case_id, { household, person });
      // Separate canonical main-person calls preserve the exact comparison
      // gate for BOTH people. The nested spouse breakdown is legacy whole DKK.
      // Pair-level prior losses, special relief and settlement are neutral in
      // this fixed fixture; this is not an adapter for swapping personal cases.
      return { case_id, input: { ...structuredClone(base), ...structuredClone(own),
        ægtefælle: { $variant: 'MedÆgtefælle', fakta: structuredClone(people[1 - person]),
          samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] } } };
    });
  });
  const output = run(['call', model, '--input', save(directory, 'input.json', envelope)]);
  save(directory, 'results.json', output);
  assert.deepEqual(output.diagnostics, []);
  assert.deepEqual(output.results.map(row => row.case_id), [...expected.keys()]);
  const sum = value => Object.values(value).reduce((a, b) => a + b, 0);
  for (const { case_id, result: r } of output.results) {
    const { household: h, person } = expected.get(case_id);
    const [bundskat, municipal, credit, total] = h.observed[person];
    const assessment = r.vurdering, exact = r.hovedskat_eksakt;
    assert.equal(assessment.alle_kontroller_gyldige, true, case_id);
    assert.equal(assessment.samlet_modeldækning_bekræftet, false, case_id);
    assert.ok(assessment.forbehold.length > 0, case_id);
    assert.deepEqual(assessment.fejl, [], case_id);
    assert.equal(r.ægtefælle.$variant, 'BeregnetÆgtefælle', case_id);
    assert.equal(r.ægtefælle.samlevende_ved_indkomstårets_udløb, true, case_id);
    assert.equal(r.skat.nettokapitalindkomst_kroner, h.interest[person] - h.expenses[person], case_id);
    assert.equal(exact.før_nedsættelser.par6_skat_øre, bundskat, `${case_id}: bundskat`);
    assert.equal(exact.før_nedsættelser.kommuneskat_øre, municipal, `${case_id}: municipal`);
    assert.equal(sum(exact.efter_personfradrag) - sum(exact.efter_par11), credit, `${case_id}: PSL11 used`);
    assert.equal(assessment.slutskat_til_sammenligning_øre, total, `${case_id}: comparison`);
    assert.equal(r.slutskat_øre, total, `${case_id}: final tax`);
    if (case_id === 'unused-spouse-credit-b') {
      const transfers = exact.udgående_overførsler;
      assert.equal(transfers.par13_indkomstfradrag_kroner, 40000);
      assert.equal(transfers.ubrugt_par6_personfradrag_øre, 619716);
      assert.equal(transfers.ubrugt_kommunalt_personfradrag_øre, 1212600);
      assert.equal(transfers.ubrugt_par11_nedslag_øre, 320000);
    }
    console.log(`${case_id}: ${total} øre matches recorded SKAT calculated tax`);
  }
});
