// Recorded anonymous 2025 form observations; offline regression, not a live
// oracle or approval of arbitrary reports. Known disagreements stay open.
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

test('employer pension observations retain matches, low-wage and shared-cap disagreements', {
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
    make('rate-low-basis', 100000, 'Pbl18Rateopsparing', false),
    make('rate-private-at-cap', 600000, 'Pbl18Rateopsparing', false),
  ];
  // Independent fictional payment: gross = net for this private contribution.
  // Together with employer net 46000, it reaches (but does not exceed) 65500.
  const mixed = template.cases.find(c => c.case_id === 'rate-private-at-cap').input;
  const privatePayment = structuredClone(mixed.lønmodtager.pension.pbl18_indbetalinger[0]);
  privatePayment.identifikation = 'fictional-private-rate-19500';
  privatePayment.indbetalingskilde = { $variant: 'Pbl18EgenIndbetaling' };
  privatePayment.betaling.beløb_kroner = 19500;
  privatePayment.betaling.arbejdsmarkedsbidrag_kroner = 0;
  mixed.lønmodtager.pension.pbl18_indbetalinger.push(privatePayment);
  const unknown = make('unknown-atp', 600000, 'Pbl18Rateopsparing', true);
  unknown.input.lønmodtager.pension.atp = { $variant: 'AtpUoplyst' };
  template.cases.push(unknown);
  const raw = run(['call', model, '--input', save('cases.json', template)]);
  const resultPath = save('results.json', raw);
  const output = JSON.parse(raw);
  assert.deepEqual(output.diagnostics, []);
  assert.deepEqual(output.results.map(c => c.case_id), template.cases.map(c => c.case_id));
  // Whole kroner except the two ore-valued tax fields. Literal observations,
  // transcribed from the specification; never used to construct model inputs.
  // The two separately recorded over-cap form rejections have no numerical
  // result and are deliberately NOT in this conformance table (see the guide).
  const observed = {
    'atp-only-low': { employment: 12669, extra: 332, taxable: 78999, municipal_ore: 1856476, total_ore: 1929080 },
    'life-atp': { employment: 55600, extra: 5852, taxable: 487648, municipal_ore: 11459728, total_ore: 21056932 },
    'rate-only': { employment: 55600, extra: 0, taxable: 493500, municipal_ore: 11597250, total_ore: 21194454 },
    'rate-atp': { employment: 55600, extra: 332, taxable: 493168, municipal_ore: 11589448, total_ore: 21186652 },
    'rate-low-basis': { employment: 12300, extra: 0, taxable: 79700, municipal_ore: 1872950, total_ore: 1945554 },
    'rate-private-at-cap': { employment: 55600, extra: 2340, taxable: 471660, municipal_ore: 11084010, total_ore: 20447019 },
  };
  // These separate expectations preserve the encoded source interpretation;
  // they are not official observations and must never be reported as matches.
  const unresolved = {
    'rate-only': { employment: 55600, extra: 5520, taxable: 487980, municipal_ore: 11467530, total_ore: 21064734 },
    'rate-atp': { employment: 55600, extra: 5852, taxable: 487648, municipal_ore: 11459728, total_ore: 21056932 },
    'rate-low-basis': { employment: 18450, extra: 5520, taxable: 68030, municipal_ore: 1598705, total_ore: 1671309 },
    'rate-private-at-cap': { employment: 55600, extra: 7860, taxable: 466140, municipal_ore: 10954290, total_ore: 20317299 },
  };
  const comparisons = [];
  for (const { case_id, result: r } of output.results) {
    const a = r.vurdering;
    assert.equal(a.samlet_modeldækning_bekræftet, false, case_id);
    assert.ok(a.forbehold.some(text => text.includes(caveat)), case_id);
    assert.ok(a.forbehold.some(text => text.includes(caveat) && text.includes('beskæftigelsesfradrag')), case_id);
    if (case_id === 'unknown-atp') {
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
    if (!Object.hasOwn(unresolved, case_id)) assert.deepEqual(actual, observed[case_id], case_id);
    else {
      // Preserve the source-backed model and make the discrepancy explicit.
      // A passing regression is NOT a claim that these observations match.
      assert.deepEqual(actual, unresolved[case_id], case_id);
      assert.equal(actual.extra - observed[case_id].extra, 5520, case_id);
      assert.equal(observed[case_id].total_ore - actual.total_ore,
        case_id === 'rate-low-basis' ? 274245 : 129720, case_id);
      if (case_id === 'rate-low-basis') {
        assert.equal(r.arbejdsfradrag_udland.grundlag.grundlag_før_udlandsafgrænsning_kroner, 150000);
        assert.equal(actual.employment - observed[case_id].employment, 6150);
      }
      if (case_id === 'rate-private-at-cap') {
        assert.equal(r.pension.arbejdsgiver_rate_resultat.bortseelsesberettiget_efter_am_kroner, 46000);
        assert.equal(r.pension.pbl18_årsresultat.rate_og_ophørende_fradrag_kroner, 19500);
        assert.equal(r.skat.personlig_indkomst_efter_am_kroner, 532500);
      }
    }
    comparisons.push({ case_id, status: Object.hasOwn(unresolved, case_id) ? 'unresolved-disagreement' : 'match',
      model: actual, observed: observed[case_id] });
  }
  assert.equal(comparisons.filter(c => c.status === 'match').length, 2);
  assert.equal(comparisons.filter(c => c.status === 'unresolved-disagreement').length, 4);
  save('comparison.json', comparisons);
  const rendered = renderPersonskatOutput(readSavedOutput(resultPath));
  assert.equal(rendered.exitCode, 2, 'unknown ATP remains attention-required');
  assert.ok(rendered.text.includes(caveat), 'the Danish viewer must retain the qualification');
  save('resultat.txt', rendered.text);
  console.log('2 official-form matches; 4 unresolved disagreements; 1 invalid-source control. Not full tax conformance.');
});
