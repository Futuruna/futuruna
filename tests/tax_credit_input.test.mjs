// Source-backed intake guidance and fictional observations, not a PDF importer
// or an independent administrative tax oracle. Never build a compiler here.
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
const source = 'https://www.retsinformation.dk/eli/lta/2024/460';
const v = ($variant, fields = {}) => ({ $variant, ...fields });
function run(args, status = 0) {
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 1200000,
    maxBuffer: 32 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, status, p.stderr); return JSON.parse(p.stdout);
}
function evidence() {
  const dir = mkdtempSync(join(tmpdir(), 'futuruna-credit-input-'));
  console.log(`Fictional credit-input evidence: ${dir}`);
  return (name, value) => {
    const path = join(dir, name);
    writeFileSync(path, JSON.stringify(value, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
    return path;
  };
}
const guidance = [
  ['b_skat_betalt', 'skullet betales', ['skattebillet', 'bankbetaling', 'restancer', 'historiske feltnavn']],
  ['par68_indbetalt', 'skullet indbetales', ['§ 68', 'bankbetaling', 'historiske feltnavn']],
  ['a_skat_og_am_indeholdt', 'indeholdt', ['A-skat', 'AM-bidrag', 'beregnede skat', 'subtotal', 'én gang']],
  ['frivillig_indbetaling_par59', 'godskrevet', ['faktisk indbetalt', 'rente', '§ 59']],
  ['tilbagebetalt_par55', '§ 55', ['positivt', 'trækker', 'ikke allerede', '§ 62']],
];
function assertGuidance(schema, prefixes) {
  const checked = [];
  for (const [prefix, suffix] of prefixes) {
    for (const [name, question, phrases] of guidance) {
      const path = `${prefix}.${name}_${suffix}`;
      const matches = schema.field_metadata.filter(f => f.path === path);
      assert.equal(matches.length, 1, path);
      const field = matches[0]; checked.push(field);
      assert.ok(field.question.includes(question), `${path}: ${field.question}`);
      for (const phrase of [...phrases, 'Ukendt er ikke 0', 'personskat-skattekreditter.md']) {
        assert.ok(field.help.includes(phrase), `${path}: ${phrase}`);
      }
      assert.equal(field.unit, suffix === 'øre' ? 'øre' : 'kr.');
      assert.equal(field.anchor, suffix === 'øre' ? 'KildeskatPar60KreditterØre' : 'KildeskatPar60Kreditter');
      const sources = schema.source_groups[field.source_group]?.map(id => schema.source_objects[id]);
      assert.ok(sources?.some(s => s.role === 'source' && JSON.stringify(s).includes(source)), path);
      assert.ok(sources.some(s => s.role === 'warning'), `${path}: source-fact boundary`);
      assert.ok(sources.some(s => s.role === 'warning'
        && JSON.stringify(s).includes('ikke-negative bruttobeløb')
        && JSON.stringify(s).includes('forskelsbeløb')
        && JSON.stringify(s).includes('absolut værdi')), `${path}: sign convention, not normalization`);
    }
  }
  return checked;
}

test('canonical credit guidance distinguishes assessed amounts from cash payments in both units', enabled, () => {
  const save = evidence();
  const schema = run(['schema', model, '--format', 'compact-json']);
  // Save actual pre/post-fix guidance even when the expectation fails.
  save('credit-guidance.json', schema.field_metadata.filter(f => f.path.startsWith('årsopgørelse.')));
  assertGuidance(schema, [
    ['årsopgørelse.MedEksaktÅrsopgørelse.kreditter', 'øre'],
    ['årsopgørelse.MedÅrsopgørelse.kreditter', 'kroner'],
  ]);
  const selector = schema.field_metadata.find(f => f.path === 'årsopgørelse.$variant');
  assert.ok(selector.help.includes('ikke slutskatten'));
  assert.ok(selector.help.includes('restskat eller overskydende skat'));
});

test('credit metadata follows reusable types through nested collection inputs', enabled, () => {
  const schema = run(['schema', 'tests/fixtures/calculation/tax_credit_metadata.calculate.runa',
    '--format', 'compact-json']);
  const fields = assertGuidance(schema, [['direct', 'øre'], ['nested.exact', 'øre'], ['nested.whole', 'kroner']]);
  for (let i = 0; i < guidance.length; i++) {
    for (const offset of [5, 10]) {
      assert.equal(fields[i].question, fields[i + offset].question);
      assert.equal(fields[i].help, fields[i + offset].help);
    }
  }
});

test('fictional assessed credits reconcile independently of bank payments and computed AM', enabled, () => {
  const save = evidence();
  const envelope = run(['template', model, '--format', 'json']);
  const base = buildFictionalCases(envelope).envelope.cases.find(c => c.case_id === 'privat-rate').input;
  base.lønmodtager.pension.pbl18_indbetalinger = [];
  // All observations are invented for this test. The existing baseline supplies
  // explicitly fictional ordinary 2025 facts, not facts inferred from a tax total.
  const observed = {
    withheld_a_tax_ore: 10500000, withheld_am_ore: 4500000,
    tax_bill_required_ore: 4000000, tax_bill_cash_paid_ore: 3000000,
    section68_required_ore: 1000000, section68_cash_paid_ore: 400000,
    section59_cash_paid_ore: 210000, section59_interest_ore: 10000,
    section55_refund_ore: 100000,
  };
  save('fictional-observations.json', observed);
  const creditNames = ['a_skat_og_am_indeholdt', 'par68_indbetalt', 'b_skat_betalt',
    'udbytteskat_modregningsberettiget', 'frivillig_indbetaling_par59', 'virksomhedsordning_beløb',
    'personskattelov_par8a_stk5_beløb', 'afskrivningslov_acontoskat', 'am_lov_par6_beløb',
    'seniornedslag', 'energiafgiftskompensation', 'tilbagebetalt_par55'];
  const credits = Object.fromEntries(creditNames.map(n => [n + '_øre', 0]));
  Object.assign(credits, {
    a_skat_og_am_indeholdt_øre: observed.withheld_a_tax_ore + observed.withheld_am_ore,
    b_skat_betalt_øre: observed.tax_bill_required_ore,
    par68_indbetalt_øre: observed.section68_required_ore,
    frivillig_indbetaling_par59_øre: observed.section59_cash_paid_ore - observed.section59_interest_ore,
    tilbagebetalt_par55_øre: observed.section55_refund_ore,
  });
  save('mapped-credits.json', credits);
  const variants = [
    ['source-correct-ore', 1094454, {}],
    ['wrong-b-skat-cash', 2094454, { b_skat_betalt_øre: observed.tax_bill_cash_paid_ore }],
    ['wrong-par68-cash', 1694454, { par68_indbetalt_øre: observed.section68_cash_paid_ore }],
    ['wrong-par59-including-interest', 1084454, { frivillig_indbetaling_par59_øre: observed.section59_cash_paid_ore }],
    ['wrong-computed-am', 794454, { a_skat_og_am_indeholdt_øre: observed.withheld_a_tax_ore + 4800000 }],
    ['source-correct-kroner', 1094454, {}],
  ];
  envelope.cases = variants.map(([case_id, , edit]) => {
    const input = structuredClone(base);
    const common = { afskrivningslov40c_acontoskat: v('UdenAfskrivningslov40CAcontoskat') };
    input.årsopgørelse = case_id === 'source-correct-kroner'
      ? v('MedÅrsopgørelse', { ...common, overført_restskat_mv_kroner: 0, øvrig_pensionsbeskatningsafgift_kroner: 0,
        kreditter: Object.fromEntries(creditNames.map(n => [n + '_kroner', credits[n + '_øre'] / 100])) })
      : v('MedEksaktÅrsopgørelse', { ...common, overført_restskat_mv_øre: 0, øvrig_pensionsbeskatningsafgift_øre: 0,
        kreditter: { ...credits, ...edit }, afregningsfakta: v('AfregnRestskat', { fakta: {
          indkomstår: 2025, oplysningspligt_ikke_rettidigt_opfyldt: false,
          påbegyndte_måneder_fra_1_september: 0, øvrige_skyldige_renter_øre: 0,
        } }) });
    return { case_id, input };
  });
  const unresolved = structuredClone(envelope.cases[0]);
  unresolved.case_id = 'unresolved-b-skat';
  unresolved.input.årsopgørelse.kreditter.b_skat_betalt_øre = null;
  envelope.cases.push(unresolved);
  const out = run(['call', model, '--input', save('cases.json', envelope)], 1);
  save('results.json', out);
  assert.deepEqual(out.results.map(c => c.case_id), variants.map(([id]) => id));
  assert.ok(out.diagnostics.length > 0);
  assert.ok(out.diagnostics.every(d => d.case_id === 'unresolved-b-skat'));
  assert.ok(out.diagnostics.some(d => d.path.includes('b_skat_betalt_øre')));
  for (const [i, { case_id, result: r }] of out.results.entries()) {
    assert.equal(r.vurdering.alle_kontroller_gyldige, true, case_id);
    assert.equal(r.vurdering.slutskat_til_sammenligning_øre, 21194454, case_id);
    assert.equal(r.årsopgørelse.resultat.restskat_øre, variants[i][1], case_id);
    assert.equal(r.årsopgørelse.resultat.modregnede_foreløbige_skatter_øre,
      21194454 - variants[i][1], case_id);
  }
  // Deliberately wrong positive scalars also pass structural/model checks.
  // Guidance reduces the intake risk; the runtime does NOT authenticate sources.
});
