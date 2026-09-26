// Fictional cases only. Test generated guidance and the existing input contract;
// this is not automatic source-document reconciliation or a second tax engine.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
const model = 'examples/danish-income-tax/personskat.calculate.runa';
const options = { skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no build or network.' };
const source = 'https://skat.dk/borger/fradrag/fradrag-for-renter';
const v = ($variant, fields = {}) => ({ $variant, ...fields });
function run(args) {
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 600000,
    maxBuffer: 32 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
}
function evidence() {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-interest-input-'));
  console.log(`Fictional interest-input evidence: ${directory}`);
  return (name, value) => {
    const path = join(directory, name);
    writeFileSync(path, JSON.stringify(value, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
    return path;
  };
}

test('generated main and spouse interest guidance shares source and amount boundaries', options, () => {
  const save = evidence();
  const schema = run(['schema', model, '--format', 'compact-json']);
  for (const [name, phrases] of [
    ['renteindtægter_kroner', ['egen andel', 'modregne', 'én gang', 'ukendt', 'Næringsstatus']],
    ['renteudgifter_kroner', ['egen andel', 'positivt', 'afdrag', 'én gang', 'ukendt', '41', '42', '44', 'provisioner',
      'minustegn', 'rettelse', 'ikke automatisk absolut værdi', 'fortegn og øre', 'total og underposter',
      'fra-bilag-til-input.md#renteudgifter-bevar-kildens-fortegn']],
  ]) {
    const pair = ['', 'ægtefælle.MedÆgtefælle.fakta.'].map(prefix => {
      const path = `${prefix}kapitalindkomst.renter.${name}`;
      const fields = schema.field_metadata.filter(field => field.path === path);
      assert.equal(fields.length, 1, path);
      return fields[0];
    });
    save(`${name}-fields.json`, pair);
    assert.equal(pair[0].question, pair[1].question, 'one question for each assessed person');
    assert.equal(pair[0].help, pair[1].help, 'spouse must not lose the input safeguards');
    for (const field of pair) {
      assert.equal(field.anchor, 'PersonskatRenteInput');
      assert.equal(field.unit, 'kr./år');
      assert.ok(field.question?.length > 0);
      for (const phrase of phrases) assert.ok(field.help.includes(phrase), `${field.path}: ${phrase}`);
      assert.ok(field.help.includes('skatdk-rentefradrag-ekstern.md'), field.path);
      const sources = schema.source_groups[field.source_group]?.map(id => schema.source_objects[id]);
      assert.ok(JSON.stringify(sources).includes(source), `${field.path}: official guidance`);
    }
  }
});

test('ordinary interest totals keep sign validation and existing netting results', options, () => {
  const save = evidence();
  const envelope = run(['template', model, '--format', 'json']);
  const base = envelope.cases[0].input;
  base.ægtefælle = { $variant: 'UdenÆgtefælle' }; // Explicit fictional absence.
  // Explicitly invented absence of other facts, not permission to use zero
  // placeholders when reading a real report with missing facts.
  Object.assign(base.lønmodtager, { skatteår: 2025, bruttoløn_kroner: 600000,
    kommune: v('København'), kirkeskat: { $variant: 'IngenKirkeskatHeleÅret' } });
  Object.assign(base.lønmodtager.pension, { fødselsdato: { år: 1990, måned: 1, dag: 1 },
    atp: v('IngenAtpIndbetalinger'),
    udbetalingsoplysninger: { for_året_komplette: true, for_foregående_år_komplette: true } });
  Object.assign(base.lønmodtager.ligningsfradrag, {
    boligjob: v('IngenBoligjobudgifter'), enlig_forsørger: v('IntetEkstraBørnetilskud'),
    arbejdsfradrag_udland: v('IngenUdlandsudelukkelseIFællesForhold', {
      dbo_hjemmehørende_udland_i_nogen_periode: false, noget_arbejde_udført_udland: null,
      nogen_udenlandsk_arbejdsgiver: null, kildereference: 'Fiktiv DBO-hjemmehørende DK hele året',
    }),
  });
  const cases = [];
  const add = (case_id, expense, income, expected, spouse = false) => {
    const input = structuredClone(base); let person = input;
    if (spouse) {
      const names = ['lønmodtager', 'kapitalindkomst', 'aktieavance', 'udenlandske_sociale_bidrag',
        'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter'];
      person = Object.fromEntries(names.map(name => [name, structuredClone(base[name])]));
      input.ægtefælle = v('MedÆgtefælle', { fakta: person, samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] });
    }
    Object.assign(person.kapitalindkomst.renter, { renteudgifter_kroner: expense, renteindtægter_kroner: income });
    cases.push({ case_id, input, expected, spouse });
  };
  add('confirmed-zero', 0, 0, 21194454);
  // Previously recorded independent official-calculator observations, preserved
  // in skatdk-rentefradrag-ekstern.md; no new external observation is claimed.
  add('expense-only', 50001, 0, 19619430);
  add('gross-income-and-expense', 51001, 1000, 19619430);
  add('negative-expense', -1, 0, null);
  add('negative-income', 0, -1, null);
  add('spouse-negative-expense', -1, 0, null, true);
  add('spouse-negative-income', 0, -1, null, true);
  envelope.cases = cases.map(({ case_id, input }) => ({ case_id, input }));
  const output = run(['call', model, '--input', save('cases.json', envelope)]);
  save('results.json', output);
  assert.deepEqual(output.diagnostics, []); assert.equal(output.results.length, cases.length);
  for (const [i, { case_id, result: r }] of output.results.entries()) {
    const c = cases[i], person = c.spouse ? r.ægtefælle.grundlag : r;
    assert.equal(case_id, c.case_id);
    assert.equal(r.vurdering.alle_kontroller_gyldige, c.expected !== null, case_id);
    assert.equal(r.vurdering.slutskat_til_sammenligning_øre, c.expected, case_id);
    assert.equal(person.kapitalindkomst.rentegrundlag_gyldigt, c.expected !== null, case_id);
  }
  console.log(JSON.stringify(output.results.map(({ case_id, result: r }) => ({ case_id,
    valid: r.vurdering.alle_kontroller_gyldige, comparison_ore: r.vurdering.slutskat_til_sammenligning_øre }))));
});
