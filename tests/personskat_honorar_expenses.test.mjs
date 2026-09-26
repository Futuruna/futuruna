// Fictional intake through the public contract. No private data or compiler build.
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
const partyearModel = 'examples/danish-income-tax/personskat-par14.calculate.runa';
const path = 'lønmodtager.personlig_indkomst.ordinære_forhold.personlige_arbejdsvederlag';
const v = $variant => ({ $variant });
const cost = amount => ({ identifikation: 'fiktivt-udgiftsbilag', indkomstår: 2025,
  art: v('PsHonorarLøbendeUdgift'), egen_udgift_kroner: amount, dokumenteret: true,
  vedrører_vederlaget: true, ikke_fratrukket_andetsteds: true });
const fee = amount => ({ identifikation: 'fiktiv-honoraraktivitet', indkomstår: 2025,
  forhold: v('PsArbejdeUdenAnsættelse'), vederlagsform: v('PsArbejdsvederlagIPenge'),
  skattepligtig_værdi_kroner: 50000,
  udgifter: { $variant: 'PsHonorarudgifterOplyst', poster: amount ? [cost(amount)] : [], fuldstændige: true } });
const rows = input => input.lønmodtager.personlig_indkomst.ordinære_forhold.personlige_arbejdsvederlag;
function run(args) {
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 600000,
    maxBuffer: 64 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
}
function save(dir, name, data) {
  const file = join(dir, name); writeFileSync(file, JSON.stringify(data), { flag: 'wx', mode: 0o600 }); return file;
}
function baseline(template) {
  const input = buildFictionalCases(template).envelope.cases[0].input;
  // Separate fictional case: source builder baseline but no private pension.
  input.lønmodtager.pension.pbl18_indbetalinger = [];
  rows(input).push(fee(0)); return input;
}

test('honorarium costs reduce personal income while preserving gross AM and work deductions', enabled, () => {
  const evidence = mkdtempSync(join(tmpdir(), 'futuruna-honorar-expenses-'));
  console.log(`Fictional honorarium expense evidence: ${evidence}`);
  const envelope = run(['template', model, '--format', 'json']);
  const base = baseline(envelope), cases = [];
  function add(case_id, valid, edit, spouse = false) {
    const input = structuredClone(base); edit(input); cases.push({ case_id, input, valid, spouse });
  }
  add('known-no-costs', true, () => {});
  add('documented-costs', true, input => { rows(input)[0] = fee(10000); });
  add('unknown-costs', false, input => { rows(input)[0].udgifter = v('PsHonorarudgifterUoplyst'); });
  add('undocumented-costs', false, input => { rows(input)[0] = fee(10000); rows(input)[0].udgifter.poster[0].dokumenteret = false; });
  add('duplicate-across-fees', false, input => {
    rows(input)[0] = fee(10000); const second = fee(10000); second.identifikation = 'anden-aktivitet'; rows(input).push(second);
  });
  add('negative-net', false, input => { rows(input)[0] = fee(46001); });
  for (const [id, amount, valid] of [['spouse-costs', 10000, true], ['spouse-negative-net', 46001, false]]) {
    add(id, valid, input => {
      const fakta = Object.fromEntries(['lønmodtager', 'kapitalindkomst', 'aktieavance', 'udenlandske_sociale_bidrag',
        'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter'].map(key => [key, structuredClone(input[key])]));
      rows(fakta)[0] = fee(amount); rows(input).length = 0;
      input.ægtefælle = { $variant: 'MedÆgtefælle', fakta, samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] };
    }, true);
  }
  envelope.cases = cases.map(({ case_id, input }) => ({ case_id, input }));
  const output = run(['call', model, '--input', save(evidence, 'cases.json', envelope)]);
  save(evidence, 'results.json', output); assert.deepEqual(output.diagnostics, []);
  assert.equal(output.results.length, cases.length);
  for (const [i, { case_id, result: r }] of output.results.entries()) {
    const expected = cases[i]; assert.equal(case_id, expected.case_id);
    assert.equal(r.vurdering.alle_kontroller_gyldige, expected.valid, case_id);
    assert.equal(r.vurdering.slutskat_til_sammenligning_øre, expected.valid ? r.slutskat_øre : null, case_id);
    if (!expected.valid) {
      const error = r.vurdering.fejl.find(f => f.sti === `${expected.spouse ? 'ægtefælle.MedÆgtefælle.fakta.' : ''}${path}.udgifter`);
      assert.ok(error, case_id);
      for (const phrase of ['ikke et lovbestemt fradragsloft', 'skattepligtig indkomst',
        'ikke automatisk i personlig indkomst', 'udkast', 'ikke sat i kraft']) {
        assert.ok(error.forklaring.includes(phrase), `${case_id}: ${phrase}`);
      }
    }
  }
  const result = id => output.results.find(r => r.case_id === id).result;
  const before = result('known-no-costs'), after = result('documented-costs');
  assert.equal(before.vurdering.slutskat_til_sammenligning_øre, 23227914, 'existing no-cost fee is unchanged');
  assert.equal(before.skat.personlig_indkomst_efter_am_kroner, 598000);
  assert.equal(after.skat.personlig_indkomst_efter_am_kroner, 588000);
  assert.equal(after.personlig_indkomst.fradrag_i_personlig_indkomst_kroner, 10000);
  for (const r of [before, after]) {
    assert.equal(r.skat.bruttoløn_kroner, 650000);
    assert.equal(r.skat.arbejdsmarkedsbidrag_kroner, 52000);
  }
  for (const key of ['beskæftigelsesfradrag_kroner', 'jobfradrag_kroner']) {
    assert.equal(typeof before.skat[key], 'number', key);
    assert.equal(after.skat[key], before.skat[key], key);
  }
  assert.deepEqual(after.arbejdsfradrag_udland.grundlag, before.arbejdsfradrag_udland.grundlag);
  assert.equal(after.arbejdsfradrag_udland.grundlag.grundlag_før_udlandsafgrænsning_kroner, 650000,
    'test the deduction basis too, not only capped deductions');
  assert.equal(result('spouse-costs').ægtefælle.grundlag.personlig_indkomst.fradrag_i_personlig_indkomst_kroner, 10000);
  assert.equal(result('spouse-costs').ægtefælle.grundlag.lønmodtager_input.øvrig_personlig_indkomst_kroner, -10000);
  assert.ok(after.slutskat_øre < before.slutskat_øre);
  assert.equal(result('negative-net').personlig_indkomst.alle_input_gyldige, false, 'propagate coverage into downstream composition');
  console.log(JSON.stringify({ gross: after.skat.bruttoløn_kroner, AM: after.skat.arbejdsmarkedsbidrag_kroner,
    personalIncome: after.skat.personlig_indkomst_efter_am_kroner, taxOre: after.slutskat_øre }));
  const schema = run(['schema', model, '--format', 'compact-json']);
  for (const prefix of ['', 'ægtefælle.MedÆgtefælle.fakta.']) {
    for (const suffix of ['udgifter.$variant', 'udgifter.PsHonorarudgifterOplyst.fuldstændige', 'udgifter.PsHonorarudgifterOplyst.poster',
      ...['identifikation', 'indkomstår', 'art', 'egen_udgift_kroner', 'dokumenteret', 'vedrører_vederlaget', 'ikke_fratrukket_andetsteds']
        .map(member => `udgifter.PsHonorarudgifterOplyst.poster.${member}`)]) {
      const key = `${prefix}${path}.${suffix}`, field = schema.field_metadata.find(f => f.path === key);
      assert.ok(field?.question && field.help, key);
      const sources = schema.source_groups[field.source_group].map(id => schema.source_objects[id]);
      assert.ok(JSON.stringify(sources).includes('oid=2048532'), key);
      assert.ok(sources.some(s => s.role === 'warning'), key);
      for (const [role, oid] of [['source', '2459224'], ['preparatory_source', '2459115'], ['currentness_source', '16080']]) {
        assert.ok(sources.some(s => s.role === role && JSON.stringify(s).includes(`oid=${oid}`)), `${key}: ${role}`);
      }
    }
  }
});

test('honorarium sources distinguish adjudicated law from unpublished practice', enabled, () => {
  const meta = run(['meta', '--json', '--type', 'HonorarudgiftKilde',
    'examples/danish-income-tax/personskat-honorarudgifter.runa']);
  assert.deepEqual(meta.diagnostics, []);
  const anchor = meta.anchors.find(a => a.label === 'personskat_honorarudgifter');
  assert.ok(anchor);
  assert.equal(anchor.references.length, 1);
  const attachments = anchor.references[0].attachments;
  for (const [role, oid] of [['source', '2459224'], ['preparatory_source', '2459115'], ['currentness_source', '16080']]) {
    const matches = attachments.filter(s => JSON.stringify(s.data).includes(`oid=${oid}`));
    assert.equal(matches.length, 1, oid);
    assert.equal(matches[0].role, role, oid);
  }
  const warning = attachments.find(s => s.role === 'warning')?.data.value;
  for (const phrase of ['ikke et lovbestemt fradragsloft', 'skattepligtig indkomst',
    'ikke automatisk i personlig indkomst', 'udkast', 'ikke sat i kraft']) {
    assert.ok(warning?.includes(phrase), phrase);
  }
  const span = meta.spans.find(s => s.label === anchor.label);
  assert.ok(span.symbols.some(s => s.name === 'personskat_honorarudgifter_netto_dækket'));
});

test('part-year composition preserves gross fees, distinct costs and the loss coverage boundary', enabled, () => {
  const evidence = mkdtempSync(join(tmpdir(), 'futuruna-honorar-partyear-'));
  console.log(`Fictional part-year honorarium evidence: ${evidence}`);
  const envelope = run(['template', partyearModel, '--entry', 'beregn_personskat_delår', '--format', 'json']);
  const person = baseline({ cases: [{ input: envelope.cases[0].input.personskat }] });
  person.lønmodtager.bruttoløn_kroner = 300000;
  envelope.cases = [0, 10000, 46001].map(amount => {
    const personskat = structuredClone(person); rows(personskat)[0] = fee(amount);
    const source = (id, field, value) => ({ identifikation: id, beregningsfelt: v(field), delårsbeløb_kroner: value,
      omregningsmetode: v('Par14ForholdsmæssigtLøbendeBeløb'), faktisk_helårsbeløb_kroner: null });
    return { case_id: `cost-${amount}`, input: { personskat, skattepligtsændring: v('FuldSkattepligtOphører'),
      skattepligtsperiode: { fra_dato: { år: 2025, måned: 1, dag: 1 }, til_dato: { år: 2025, måned: 6, dag: 30 } },
      valg_afgivet_ved_oplysninger: false, omvalg_dato: null, helårsgrundlag: v('AfledtFraIdentificeredeKilder'),
      // Explicit fiction: recurring fee and costs; no inference from employment duration.
      kilder: [source('loen-og-honorar', 'Par14Bruttoløn', 350000), source('honorarudgift', 'Par14ØvrigPersonligIndkomst', -amount)] } };
  });
  const output = run(['call', partyearModel, '--entry', 'beregn_personskat_delår', '--input', save(evidence, 'cases.json', envelope)]);
  save(evidence, 'results.json', output); assert.deepEqual(output.diagnostics, []);
  assert.equal(output.results.length, 3);
  for (const { case_id, result: r } of output.results) {
    const valid = case_id !== 'cost-46001';
    assert.equal(r.input_gyldigt, valid, case_id);
    assert.equal(r.vurdering.slutskat_til_sammenligning_øre, valid ? r.slutskat_efter_par14_øre : null, case_id);
    if (valid) assert.equal(r.arbejdsmarkedsbidrag_for_delåret_øre, 2800000);
    else assert.equal(r.delårsresultat.personlig_indkomst.alle_input_gyldige, false);
  }
  assert.equal(output.results[1].result.delårsinput.øvrig_personlig_indkomst_kroner, -10000);
  assert.ok(output.results[1].result.slutskat_efter_par14_øre < output.results[0].result.slutskat_efter_par14_øre);
});
