// Gross credit-input convention, not a ban on signed income or a complete
// correction-history model. Only fictional facts; one worker, no compiler build.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { buildFictionalCases } from '../examples/danish-income-tax/bilag-demo.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
const enabled = { skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no build or network.' };
const model = 'examples/danish-income-tax/personskat.calculate.runa';
const names = ['a_skat_og_am_indeholdt', 'par68_indbetalt', 'b_skat_betalt',
  'udbytteskat_modregningsberettiget', 'frivillig_indbetaling_par59', 'virksomhedsordning_beløb',
  'personskattelov_par8a_stk5_beløb', 'afskrivningslov_acontoskat', 'am_lov_par6_beløb',
  'seniornedslag', 'energiafgiftskompensation', 'tilbagebetalt_par55'];
const v = ($variant, fields = {}) => ({ $variant, ...fields });
const credits = (suffix, value = 0) => Object.fromEntries(names.map(n => [n + '_' + suffix, value]));
function run(args) {
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 1200000,
    maxBuffer: 32 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
}
function call(entry, envelope) {
  const dir = mkdtempSync(join(tmpdir(), 'futuruna-credit-signs-'));
  console.log(`Fictional credit-sign evidence: ${dir}`);
  const save = (name, value) => {
    const path = join(dir, name);
    writeFileSync(path, JSON.stringify(value), { flag: 'wx', mode: 0o600 }); return path;
  };
  const out = run(['call', entry, '--input', save('cases.json', envelope)]);
  save('results.json', out); assert.deepEqual(out.diagnostics, []);
  assert.deepEqual(out.results.map(c => c.case_id), envelope.cases.map(c => c.case_id));
  return out.results;
}

test('each gross credit and refund field has the same sign boundary in both units', enabled, () => {
  const entry = 'tests/fixtures/calculation/tax_credit_signs.calculate.runa';
  const envelope = run(['template', entry, '--format', 'json']);
  // Fail if the actual type gains a field that this boundary matrix overlooks.
  assert.deepEqual(Object.keys(envelope.cases[0].input.exact).sort(), Object.keys(credits('øre')).sort());
  assert.deepEqual(Object.keys(envelope.cases[0].input.whole).sort(), Object.keys(credits('kroner')).sort());
  const expected = [];
  envelope.cases = [0, 1].map(n => {
    expected.push({ exact_nonnegative: true, whole_nonnegative: true });
    return { case_id: `all-${n}`, input: { exact: credits('øre', n), whole: credits('kroner', n) } };
  });
  for (const [branch, suffix] of [['exact', 'øre'], ['whole', 'kroner']]) {
    for (const name of names) {
      const input = { exact: credits('øre'), whole: credits('kroner') };
      input[branch][name + '_' + suffix] = -1;
      envelope.cases.push({ case_id: `${branch}-${name}`, input });
      expected.push({ exact_nonnegative: branch !== 'exact', whole_nonnegative: branch !== 'whole' });
    }
  }
  const results = call(entry, envelope);
  results.forEach(({ result, case_id }, i) => assert.deepEqual(result, expected[i], case_id));
});

test('canonical annual assessment withholds negative gross credits without rewriting raw facts', enabled, () => {
  const envelope = run(['template', model, '--format', 'json']);
  const base = buildFictionalCases(envelope).envelope.cases.find(c => c.case_id === 'privat-rate').input;
  base.lønmodtager.pension.pbl18_indbetalinger = [];
  const cases = [
    ['positive-refund', 'øre', 100000, 0, true, 6294454],
    ['negative-refund-ore', 'øre', -100000, 0, false, 6094454],
    ['negative-refund-kroner', 'kroner', -1000, 0, false, 6094454],
    ['negative-b-skat', 'øre', 0, -1, false, 6194455],
  ];
  envelope.cases = cases.map(([case_id, unit, refund, bTax]) => {
    const input = structuredClone(base), k = credits(unit), whole = unit === 'kroner';
    k['a_skat_og_am_indeholdt_' + unit] = whole ? 150000 : 15000000;
    k['tilbagebetalt_par55_' + unit] = refund;
    k['b_skat_betalt_' + unit] = bTax;
    const common = { kreditter: k, afskrivningslov40c_acontoskat: v('UdenAfskrivningslov40CAcontoskat') };
    input.årsopgørelse = whole
      ? v('MedÅrsopgørelse', { ...common, overført_restskat_mv_kroner: 0, øvrig_pensionsbeskatningsafgift_kroner: 0 })
      : v('MedEksaktÅrsopgørelse', { ...common, overført_restskat_mv_øre: 0, øvrig_pensionsbeskatningsafgift_øre: 0,
        afregningsfakta: v('AfregnRestskat', { fakta: { indkomstår: 2025,
          oplysningspligt_ikke_rettidigt_opfyldt: false, påbegyndte_måneder_fra_1_september: 0,
          øvrige_skyldige_renter_øre: 0 } }) });
    return { case_id, input };
  });
  const results = call(model, envelope);
  results.forEach(({ case_id, result: r }, i) => {
    const [, unit, refund, bTax, valid, rawRestskat] = cases[i];
    const a = r.vurdering;
    assert.equal(a.alle_kontroller_gyldige, valid, case_id);
    assert.equal(a.slutskat_til_sammenligning_øre, valid ? 21194454 : null, case_id);
    assert.equal(a.status.$variant, valid ? 'BeregnetMedForbehold' : 'UgyldigtBeregningsgrundlag');
    const control = a.kontrolgrundlag.beregning.filter(c => c.sti === 'årsopgørelse.kreditter');
    assert.equal(control.length, 1); assert.equal(control[0].gyldig, valid);
    assert.equal(a.kontrolgrundlag.afregning.every(c => c.gyldig), true, 'not a payment-stage sign check');
    assert.equal(r.slutskat_øre, 21194454, 'raw tax arithmetic unchanged');
    assert.equal(r.årsopgørelse.resultat.restskat_øre, rawRestskat, 'raw settlement is diagnostic, not normalized');
    assert.equal(r.årsopgørelse.input.kreditter.tilbagebetalt_par55_øre, unit === 'kroner' ? refund * 100 : refund);
    assert.equal(r.årsopgørelse.input.kreditter.b_skat_betalt_øre, bTax);
  });
});
