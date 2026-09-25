// Recorded anonymous 2025 form observations; offline regression, not a live
// oracle or approval of arbitrary reports. Two known disagreements stay open.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { buildFictionalCases } from '../examples/danish-income-tax/bilag-demo.mjs';
import { readSavedOutput } from '../examples/danish-income-tax/resultat-visning.mjs';
import { renderPersonskatOutput } from '../examples/danish-income-tax/personskat-resultat.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
const model = 'examples/danish-income-tax/personskat.calculate.runa';
const caveat = 'skatdk-arbejdsgiverpension-ekstern.md';

test('employer pension observations distinguish two matches from two unresolved disagreements', {
  skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build or network.',
}, () => {
  const evidence = mkdtempSync(join(tmpdir(), 'futuruna-employer-pension-'));
  console.log(`Fictional employer pension evidence: ${evidence}`);
  const save = (name, value) => {
    const path = join(evidence, name);
    writeFileSync(path, typeof value === 'string' ? value : JSON.stringify(value, null, 2) + '\n',
      { flag: 'wx', mode: 0o600 });
    return path;
  };
  const run = (args) => {
    const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 600000,
      maxBuffer: 32 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
    assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return p.stdout;
  };
  const template = JSON.parse(run(['template', model, '--format', 'json']));
  // Reuse only the explicitly fictional baseline facts, not expected tax totals.
  const baseline = buildFictionalCases(template).envelope.cases.find(c => c.case_id === 'arbejdsgiver-atp').input;
  const make = (case_id, salary, kind, atp) => {
    const input = structuredClone(baseline);
    input.lønmodtager.bruttoløn_kroner = salary;
    if (kind === null) input.lønmodtager.pension.pbl18_indbetalinger = [];
    else input.lønmodtager.pension.pbl18_indbetalinger[0].ordning = { $variant: kind };
    if (!atp) input.lønmodtager.pension.atp = { $variant: 'IngenAtpIndbetalinger' };
    return { case_id, input };
  };
  template.cases = [
    make('atp-only-low', 100000, null, true),
    make('life-atp', 600000, 'Pbl18LivsvarigLivrente', true),
    make('rate-only', 600000, 'Pbl18Rateopsparing', false),
    make('rate-atp', 600000, 'Pbl18Rateopsparing', true),
    make('unknown-atp', 600000, 'Pbl18Rateopsparing', true),
  ];
  template.cases[4].input.lønmodtager.pension.atp = { $variant: 'AtpUoplyst' };
  const raw = run(['call', model, '--input', save('cases.json', template)]);
  const resultPath = save('results.json', raw);
  const output = JSON.parse(raw);
  assert.deepEqual(output.diagnostics, []);
  assert.deepEqual(output.results.map(c => c.case_id), template.cases.map(c => c.case_id));
  // Whole kroner except the two ore-valued tax fields. Literal observations,
  // transcribed from the specification; never used to construct model inputs.
  const observed = [
    { employment: 12669, extra: 332, taxable: 78999, municipal_ore: 1856476, total_ore: 1929080 },
    { employment: 55600, extra: 5852, taxable: 487648, municipal_ore: 11459728, total_ore: 21056932 },
    { employment: 55600, extra: 0, taxable: 493500, municipal_ore: 11597250, total_ore: 21194454 },
    { employment: 55600, extra: 332, taxable: 493168, municipal_ore: 11589448, total_ore: 21186652 },
  ];
  const comparisons = [];
  for (const [i, { case_id, result: r }] of output.results.entries()) {
    const a = r.vurdering;
    assert.equal(a.samlet_modeldækning_bekræftet, false, case_id);
    assert.ok(a.forbehold.some(text => text.includes(caveat)), case_id);
    if (i === 4) {
      assert.equal(a.alle_kontroller_gyldige, false);
      assert.equal(a.slutskat_til_sammenligning_øre, null);
      continue;
    }
    assert.equal(a.alle_kontroller_gyldige, true, case_id);
    assert.equal(a.status.$variant, 'BeregnetMedForbehold', case_id);
    assert.deepEqual(a.fejl, [], case_id);
    const actual = { employment: r.skat.beskæftigelsesfradrag_kroner,
      extra: r.skat.ekstra_pensionsfradrag_kroner,
      taxable: r.skat.almindelig_skattepligtig_indkomst_kroner,
      municipal_ore: r.hovedskat_eksakt.før_nedsættelser.kommuneskat_øre,
      total_ore: a.slutskat_til_sammenligning_øre };
    if (i < 2) assert.deepEqual(actual, observed[i], case_id);
    else {
      // Preserve the source-backed model and make the discrepancy explicit.
      // A passing regression is NOT a claim that these observations match.
      assert.equal(actual.extra, i === 2 ? 5520 : 5852, case_id);
      assert.equal(actual.total_ore, i === 2 ? 21064734 : 21056932, case_id);
      assert.equal(actual.extra - observed[i].extra, 5520, case_id);
      assert.equal(observed[i].total_ore - actual.total_ore, 129720, case_id);
    }
    comparisons.push({ case_id, status: i < 2 ? 'match' : 'unresolved-disagreement', model: actual, observed: observed[i] });
  }
  save('comparison.json', comparisons);
  const rendered = renderPersonskatOutput(readSavedOutput(resultPath));
  assert.equal(rendered.exitCode, 2, 'unknown ATP remains attention-required');
  assert.ok(rendered.text.includes(caveat), 'the Danish viewer must retain the qualification');
  save('resultat.txt', rendered.text);
  console.log('2 official-form matches; 2 unresolved disagreements; 1 invalid-source control. Not full tax conformance.');
});
