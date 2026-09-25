// Fictional source facts; deterministic annual composition, not a legal oracle.
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
const enabled = { skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no build, network or personal files.' };
const model = 'examples/danish-income-tax/personskat.calculate.runa';
const partyearModel = 'examples/danish-income-tax/personskat-par14.calculate.runa';
const path = 'lønmodtager.personlig_indkomst.ordinære_forhold.personlige_arbejdsvederlag';
const v = $variant => ({ $variant });
const rows = input => input.lønmodtager.personlig_indkomst.ordinære_forhold.personlige_arbejdsvederlag;
const setRows = (input, values) => { input.lønmodtager.personlig_indkomst.ordinære_forhold.personlige_arbejdsvederlag = values; };
const fee = (relation = 'PsArbejdeUdenAnsættelse', id = 'fiktivt-vederlag') => ({
  identifikation: id, indkomstår: 2025, forhold: v(relation),
  vederlagsform: v('PsArbejdsvederlagIPenge'), skattepligtig_værdi_kroner: 50000,
});
function run(args) {
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 600000,
    maxBuffer: 64 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
}
function save(directory, name, value) {
  const file = join(directory, name);
  writeFileSync(file, JSON.stringify(value, null, 2) + '\n', { flag: 'wx', mode: 0o600 }); return file;
}
function baseline(envelope) {
  const input = buildFictionalCases(envelope).envelope.cases[0].input;
  // New fiction uses the builder's explicit full-year DK baseline, but no
  // private pension. Do not treat its original source ledger as these cases.
  input.lønmodtager.pension.pbl18_indbetalinger = [];
  return input;
}
function spouseOf(input) {
  return Object.fromEntries(['lønmodtager', 'kapitalindkomst', 'aktieavance',
    'udenlandske_sociale_bidrag', 'cfc', 'skatteforhold', 'underskudsforhold',
    'ejendomsskatter'].map(key => [key, structuredClone(input[key])]));
}

test('annual assessment withholds misplaced work income without changing correctly routed fees', enabled, () => {
  const evidence = mkdtempSync(join(tmpdir(), 'futuruna-work-income-route-'));
  console.log(`Fictional work-income evidence: ${evidence}`);
  const envelope = run(['template', model, '--format', 'json']);
  const base = baseline(envelope), cases = [];
  const add = (case_id, valid, edit, spouse = false) => {
    const input = structuredClone(base); edit(input); cases.push({ case_id, input, valid, spouse });
  };
  add('baseline', true, () => {});
  add('honorar', true, input => setRows(input, [fee()]));
  for (const [id, relation] of [['employment', 'PsArbejdeIAnsættelse'], ['business', 'PsSelvstændigtArbejde']]) {
    add(`${id}-in-fee-route`, false, input => setRows(input, [fee(relation)]));
    add(`spouse-${id}-in-fee-route`, false, input => {
      const fakta = spouseOf(input); setRows(fakta, [fee(relation)]);
      input.ægtefælle = { $variant: 'MedÆgtefælle', fakta, samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] };
    }, true);
  }
  add('mixed-routes', false, input => setRows(input, [fee(), fee('PsArbejdeIAnsættelse', 'fiktiv-loen')]));
  add('duplicate-id', false, input => setRows(input, [fee(), fee()]));
  add('wrong-year', false, input => { setRows(input, [fee()]); rows(input)[0].indkomstår = 2024; });
  add('spouse-honorar', true, input => {
    const fakta = spouseOf(input); setRows(fakta, [fee()]);
    input.ægtefælle = { $variant: 'MedÆgtefælle', fakta, samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] };
  }, true);
  envelope.cases = cases.map(({ case_id, input }) => ({ case_id, input }));
  const output = run(['call', model, '--input', save(evidence, 'cases.json', envelope)]);
  save(evidence, 'results.json', output);
  assert.deepEqual(output.diagnostics, []);
  assert.equal(output.results.length, cases.length);
  for (const [index, { case_id, result: r }] of output.results.entries()) {
    const expected = cases[index]; assert.equal(case_id, expected.case_id);
    assert.equal(r.vurdering.alle_kontroller_gyldige, expected.valid, case_id);
    assert.equal(r.vurdering.slutskat_til_sammenligning_øre, expected.valid ? r.slutskat_øre : null, case_id);
    assert.equal(r.vurdering.samlet_modeldækning_bekræftet, false);
    if (!expected.valid) {
      const inputPath = `${expected.spouse ? 'ægtefælle.MedÆgtefælle.fakta.' : ''}${path}`;
      assert.ok(r.vurdering.fejl.some(f => f.sti === inputPath && f.forklaring.includes('ikke skattefri')
        && f.forklaring.includes('personskat-honorar.md')), case_id);
    }
    if (!expected.spouse && case_id.endsWith('-in-fee-route')) {
      const leaf = r.personlig_indkomst.ordinære_forhold.personlige_arbejdsvederlag[0];
      assert.equal(leaf.$variant, 'PersonligtArbejdsvederlagUdenForPar2Stk1Nr2');
      assert.deepEqual(leaf.fakta, rows(expected.input)[0], 'do not relabel the source facts');
      assert.equal(r.personlig_indkomst.alle_input_gyldige, false);
    }
  }
  const result = id => output.results.find(row => row.case_id === id).result;
  assert.equal(result('baseline').vurdering.slutskat_til_sammenligning_øre, 21194454);
  assert.equal(result('honorar').vurdering.slutskat_til_sammenligning_øre, 23227914);
  assert.equal(result('honorar').skat.bruttoløn_kroner, 650000);
  assert.equal(result('honorar').skat.arbejdsmarkedsbidrag_kroner, 52000);
  assert.equal(result('honorar').skat.personlig_indkomst_efter_am_kroner, 598000);
  assert.equal(result('mixed-routes').personlig_indkomst.ordinære_forhold.arbejdsmarkedsbidragsgrundlag_tillæg_kroner, 50000);

  const schema = run(['schema', model, '--format', 'compact-json']);
  for (const prefix of ['', 'ægtefælle.MedÆgtefælle.fakta.']) {
    for (const [member, phrases] of [
      ['', ['B-indkomst er ikke én retlig kategori', 'pr. særskilt vederlag']],
      ['.forhold', ['ikke automatisk omplacering', 'Bevar kildefakta', 'ukendt klassifikation']],
      ['.skattepligtig_værdi_kroner', ['bruttovederlaget', 'B-skat', 'honorarudgiftsfelt', 'én gang']],
    ]) {
      const field = schema.field_metadata.find(f => f.path === `${prefix}${path}${member}`);
      assert.ok(field?.question, `${prefix}${path}${member}`);
      for (const phrase of [...phrases, 'personskat-honorar.md']) assert.ok(field.help.includes(phrase), phrase);
      const sources = schema.source_groups[field.source_group].map(id => schema.source_objects[id]);
      assert.ok(JSON.stringify(sources).includes('https://info.skat.dk/data.aspx?oid=1976769'));
    }
  }
});

test('misplaced work income also invalidates the final part-year assessment', enabled, () => {
  const evidence = mkdtempSync(join(tmpdir(), 'futuruna-work-income-partyear-'));
  console.log(`Fictional part-year work-income evidence: ${evidence}`);
  const envelope = run(['template', partyearModel, '--entry', 'beregn_personskat_delår', '--format', 'json']);
  const person = baseline({ cases: [{ input: envelope.cases[0].input.personskat }] });
  person.lønmodtager.bruttoløn_kroner = 300000;
  const base = { personskat: person, skattepligtsændring: v('FuldSkattepligtOphører'),
    skattepligtsperiode: { fra_dato: { år: 2025, måned: 1, dag: 1 }, til_dato: { år: 2025, måned: 6, dag: 30 } },
    valg_afgivet_ved_oplysninger: false, omvalg_dato: null,
    kilder: [{ identifikation: 'fiktiv-loen', beregningsfelt: v('Par14Bruttoløn'), delårsbeløb_kroner: 300000,
      omregningsmetode: v('Par14ForholdsmæssigtLøbendeBeløb'), faktisk_helårsbeløb_kroner: null }],
    helårsgrundlag: v('AfledtFraIdentificeredeKilder') };
  envelope.cases = [['baseline', null], ['employment', 'PsArbejdeIAnsættelse'], ['business', 'PsSelvstændigtArbejde']]
    .map(([case_id, relation]) => { const input = structuredClone(base);
      if (relation) setRows(input.personskat, [fee(relation)]); return { case_id, input }; });
  const output = run(['call', partyearModel, '--entry', 'beregn_personskat_delår', '--input', save(evidence, 'cases.json', envelope)]);
  save(evidence, 'results.json', output);
  assert.deepEqual(output.diagnostics, []); assert.equal(output.results.length, 3);
  for (const { case_id, result: r } of output.results) {
    const valid = case_id === 'baseline';
    assert.equal(r.input_gyldigt, valid, case_id);
    assert.equal(r.vurdering.alle_kontroller_gyldige, valid, case_id);
    assert.equal(r.vurdering.slutskat_til_sammenligning_øre, valid ? 10383101 : null, case_id);
    if (!valid) assert.ok(r.delårsresultat.vurdering.fejl.some(f => f.sti === path));
  }
});
