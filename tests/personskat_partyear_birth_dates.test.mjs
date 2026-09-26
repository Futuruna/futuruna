// Same-person facts across PSL14 bases; fictional inputs, no SKAT tax oracle.
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
const model = 'examples/danish-income-tax/personskat-par14.calculate.runa';
const enabled = { skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no build or network.' };
const v = ($variant, fields = {}) => ({ $variant, ...fields });
function run(args) {
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 1200000,
    maxBuffer: 64 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr); return JSON.parse(p.stdout);
}
function save(directory, name, data) {
  const file = join(directory, name);
  writeFileSync(file, JSON.stringify(data, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
  return file;
}

test('period and documented annual bases preserve each persons birth date', enabled, async t => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-partyear-birth-'));
  console.log(`Fictional birth-date evidence: ${directory}`);
  const envelope = run(['template', model, '--format', 'json']);
  const person = buildFictionalCases({ cases: [{ input: envelope.cases[0].input.personskat }] })
    .envelope.cases[0].input;
  // New fiction: one 1990-born person arrives July1 2026, wage200000 in the
  // period, independently documented representative annual wage400000.
  // No spouse, pension/ATP, benefits, property, losses or other income/costs.
  person.lønmodtager.pension.pbl18_indbetalinger = [];
  person.lønmodtager.skatteår = 2026;
  person.lønmodtager.bruttoløn_kroner = 200000;
  const annual = structuredClone(person);
  annual.lønmodtager.bruttoløn_kroner = 400000;
  const baseline = { personskat: person, skattepligtsændring: v('FuldSkattepligtIndtræder'),
    skattepligtsperiode: { fra_dato: { år: 2026, måned: 7, dag: 1 }, til_dato: { år: 2026, måned: 12, dag: 31 } },
    valg_afgivet_ved_oplysninger: false, omvalg_dato: null,
    kilder: [{ identifikation: 'fictional-salary', beregningsfelt: v('Par14Bruttoløn'), delårsbeløb_kroner: 200000,
      omregningsmetode: v('Par14DokumenteretRetvisendeHelårsbeløb', { helårsbeløb_kroner: 400000 }), faktisk_helårsbeløb_kroner: null }],
    helårsgrundlag: v('DokumenteretHelårsPersonskat', { personskat: annual }) };
  const cases = [{ case_id: 'same-birth', input: baseline, valid: true }];
  function add(case_id, from, edit, valid = false) {
    const input = structuredClone(from); edit(input);
    cases.push({ case_id, input, valid });
  }
  add('wrong-birth-year', baseline, x => { x.helårsgrundlag.personskat.lønmodtager.pension.fødselsdato.år = 1960; });
  add('wrong-birth-day', baseline, x => { x.helårsgrundlag.personskat.lønmodtager.pension.fødselsdato.dag = 2; });
  const couple = structuredClone(baseline);
  for (const [p, wage] of [[couple.personskat, 300000], [couple.helårsgrundlag.personskat, 600000]]) {
    const facts = Object.fromEntries(personFactFields.map(key => [key, structuredClone(p[key])]));
    // Spouses may have different birth dates. Compare each person only with
    // the corresponding person in the other basis, never with their spouse.
    facts.lønmodtager.pension.fødselsdato = { år: 1980, måned: 3, dag: 15 };
    facts.lønmodtager.bruttoløn_kroner = wage;
    p.ægtefælle = v('MedÆgtefælle', { fakta: facts,
      samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] });
  }
  cases.push({ case_id: 'different-spouses-consistent-bases', input: couple, valid: true });
  add('wrong-spouse-birth-year', couple, x => {
    x.helårsgrundlag.personskat.ægtefælle.fakta.lønmodtager.pension.fødselsdato.år = 1970;
  });
  add('wrong-spouse-birth-month', couple, x => {
    x.helårsgrundlag.personskat.ægtefælle.fakta.lønmodtager.pension.fødselsdato.måned = 4;
  });
  add('derived-basis', baseline, x => { x.helårsgrundlag = v('AfledtFraIdentificeredeKilder'); }, true);
  envelope.cases = cases.map(({ case_id, input }) => ({ case_id, input }));
  const output = run(['call', model, '--input', save(directory, 'input.json', envelope)]);
  save(directory, 'results.json', output);
  assert.deepEqual(output.diagnostics, []);
  assert.deepEqual(output.results.map(row => row.case_id), envelope.cases.map(row => row.case_id));
  for (const { case_id, result: r } of output.results) console.log(JSON.stringify({ case_id,
    valid: r.input_gyldigt, comparison: r.vurdering.slutskat_til_sammenligning_øre,
    faults: r.vurdering.fejl, annualSeniorDeduction: r.helårsberegning.seniorbeskæftigelsesfradrag_kroner }));
  for (const [i, c] of cases.entries()) await t.test(c.case_id, () => {
    const r = output.results[i].result, a = r.vurdering;
    assert.equal(r.input_gyldigt, c.valid);
    assert.equal(r.helårsgrundlag_gyldigt, c.valid);
    assert.equal(a.alle_kontroller_gyldige, c.valid);
    assert.equal(a.samlet_modeldækning_bekræftet, false);
    assert.equal(a.status.$variant, c.valid ? 'BeregnetMedForbehold' : 'UgyldigtBeregningsgrundlag');
    assert.deepEqual(r.delårsinput.pensionsfradrag.fødselsdato, c.input.personskat.lønmodtager.pension.fødselsdato);
    const annualPerson = c.input.helårsgrundlag.personskat ?? c.input.personskat;
    assert.deepEqual(r.helårsinput.pensionsfradrag.fødselsdato, annualPerson.lønmodtager.pension.fødselsdato,
      'a conflicting source date is not silently overwritten');
    if (c.valid) {
      assert.deepEqual(a.fejl, []);
      assert.equal(a.slutskat_til_sammenligning_øre, r.slutskat_efter_par14_øre);
      assert.ok(a.slutskat_til_sammenligning_øre > 0);
    } else {
      assert.equal(a.slutskat_til_sammenligning_øre, null);
      assert.ok(a.fejl.some(f => f.sti === 'helårsgrundlag'
        && f.forklaring.includes('fødselsdato') && f.forklaring.includes('ægtefælle')));
    }
  });
  const byId = new Map(output.results.map(row => [row.case_id, row.result]));
  // Recorded pre-fix model control, not independently observed SKAT tax.
  assert.equal(byId.get('same-birth').vurdering.slutskat_til_sammenligning_øre, 6553336);
  assert.equal(byId.get('same-birth').helårsberegning.seniorbeskæftigelsesfradrag_kroner, 0);
  assert.equal(byId.get('wrong-birth-year').helårsberegning.seniorbeskæftigelsesfradrag_kroner, 5600,
    'retain diagnostic arithmetic, but withhold it from a usable comparison');
  assert.equal(byId.get('derived-basis').vurdering.slutskat_til_sammenligning_øre,
    byId.get('same-birth').vurdering.slutskat_til_sammenligning_øre);
});

test('generated annual-basis guidance distinguishes income conversion from person facts', enabled, () => {
  const schema = run(['schema', model, '--format', 'compact-json']);
  const fields = schema.field_metadata.filter(f => f.path === 'helårsgrundlag.$variant');
  assert.equal(fields.length, 1);
  const field = fields[0];
  for (const phrase of ['fødselsdato', 'dag og måned', 'ægtefælle',
    'Indkomster kan være forskellige', 'ikke personernes identitet']) {
    assert.ok(field.help.includes(phrase), phrase);
  }
  const sources = schema.source_groups[field.source_group].map(id => schema.source_objects[id]);
  assert.ok(sources.some(s => s.role === 'source' && JSON.stringify(s).includes('2021/1284')));
  assert.ok(sources.some(s => s.role === 'guidance' && JSON.stringify(s).includes('oid=1977388')));
});
