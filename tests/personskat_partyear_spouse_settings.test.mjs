// Fictional same-year source consistency, not an independent SKAT tax oracle.
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

test('part-year bases retain the corresponding spouses annual tax settings', enabled, async t => {
  const directory = mkdtempSync(join(tmpdir(), 'futuruna-partyear-spouse-settings-'));
  console.log(`Fictional spouse-setting evidence: ${directory}`);
  const envelope = run(['template', model, '--format', 'json']);
  const person = buildFictionalCases({ cases: [{ input: envelope.cases[0].input.personskat }] })
    .envelope.cases[0].input;
  // New fiction, not the demo's source ledger: 2026 July1 arrival, own period/
  // representative annual wage200000/400000, spouse20000/40000. Both born1990,
  // Copenhagen, church all year, year-end cohabitation, no pension/ATP,
  // property/other income/costs/losses/relief; ordinary fully liable status.
  person.lønmodtager.pension.pbl18_indbetalinger = [];
  person.lønmodtager.skatteår = 2026;
  person.lønmodtager.bruttoløn_kroner = 200000;
  person.lønmodtager.kirkeskat = v('KirkeskatHeleÅret');
  const annual = structuredClone(person);
  annual.lønmodtager.bruttoløn_kroner = 400000;
  for (const [p, wage] of [[person, 20000], [annual, 40000]]) {
    const facts = Object.fromEntries(personFactFields.map(key => [key, structuredClone(p[key])]));
    facts.lønmodtager.bruttoløn_kroner = wage;
    p.ægtefælle = v('MedÆgtefælle', { fakta: facts,
      samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] });
  }
  const baseline = { personskat: person, skattepligtsændring: v('FuldSkattepligtIndtræder'),
    skattepligtsperiode: { fra_dato: { år: 2026, måned: 7, dag: 1 }, til_dato: { år: 2026, måned: 12, dag: 31 } },
    valg_afgivet_ved_oplysninger: false, omvalg_dato: null,
    kilder: [{ identifikation: 'fictional-salary', beregningsfelt: v('Par14Bruttoløn'), delårsbeløb_kroner: 200000,
      omregningsmetode: v('Par14DokumenteretRetvisendeHelårsbeløb', { helårsbeløb_kroner: 400000 }), faktisk_helårsbeløb_kroner: null }],
    helårsgrundlag: v('DokumenteretHelårsPersonskat', { personskat: annual }) };
  const cases = [{ case_id: 'consistent', input: baseline, valid: true }];
  const spouse = p => p.ægtefælle.fakta.lønmodtager;
  function add(case_id, edit, valid = false) {
    const input = structuredClone(baseline); edit(input);
    cases.push({ case_id, input, valid });
  }
  add('conflicting-municipality', x => { spouse(x.helårsgrundlag.personskat).kommune = v('Aarhus'); });
  add('conflicting-church', x => { spouse(x.helårsgrundlag.personskat).kirkeskat = v('IngenKirkeskatHeleÅret'); });
  add('conflicting-allowance-status', x => { spouse(x.helårsgrundlag.personskat).personfradrag_alder_status = v('Under18Ugift'); });
  add('different-spouses-consistent-bases', x => {
    for (const p of [x.personskat, x.helårsgrundlag.personskat]) {
      spouse(p).kommune = v('Aarhus');
      spouse(p).kirkeskat = v('IngenKirkeskatHeleÅret');
    }
  }, true);
  envelope.cases = cases.map(({ case_id, input }) => ({ case_id, input }));
  const output = run(['call', model, '--input', save(directory, 'input.json', envelope)]);
  save(directory, 'results.json', output);
  assert.deepEqual(output.diagnostics, []);
  assert.deepEqual(output.results.map(row => row.case_id), envelope.cases.map(row => row.case_id));
  for (const { case_id, result: r } of output.results) console.log(JSON.stringify({ case_id,
    valid: r.input_gyldigt, comparison: r.vurdering.slutskat_til_sammenligning_øre,
    failedPaths: r.vurdering.fejl.map(f => f.sti) }));
  for (const [i, c] of cases.entries()) await t.test(c.case_id, () => {
    const r = output.results[i].result, a = r.vurdering;
    assert.equal(r.input_gyldigt, c.valid);
    assert.equal(r.helårsgrundlag_gyldigt, c.valid);
    assert.equal(a.alle_kontroller_gyldige, c.valid);
    assert.equal(a.samlet_modeldækning_bekræftet, false);
    assert.equal(a.status.$variant, c.valid ? 'BeregnetMedForbehold' : 'UgyldigtBeregningsgrundlag');
    if (c.valid) {
      assert.deepEqual(a.fejl, []);
      assert.equal(a.slutskat_til_sammenligning_øre, r.slutskat_efter_par14_øre);
      assert.ok(a.slutskat_til_sammenligning_øre > 0);
      assert.equal(a.slutskat_til_sammenligning_øre, 6280791,
        'recorded model control, not independently observed administrative tax');
    } else {
      assert.equal(a.slutskat_til_sammenligning_øre, null);
      const field = { 'conflicting-municipality': 'kommune', 'conflicting-church': 'kirkeskat',
        'conflicting-allowance-status': 'personfradrag_alder_status' }[c.case_id];
      assert.ok(a.fejl.some(f => f.sti === 'helårsgrundlag'
        && f.forklaring.includes(`lønmodtager.${field}`)));
    }
  });
});

test('annual-basis guidance preserves source traces and permits differences between spouses', enabled, () => {
  const schema = run(['schema', model, '--format', 'compact-json']);
  const fields = schema.field_metadata.filter(f => f.path === 'helårsgrundlag.$variant');
  assert.equal(fields.length, 1);
  const field = fields[0];
  for (const phrase of ['tilsvarende ægtefælle', 'lønmodtager.kommune', 'lønmodtager.kirkeskat',
    'lønmodtager.personfradrag_alder_status', 'årets skattekommune', 'medlemsperioder',
    'Ægtefællerne kan have forskellige', 'Ens oplysninger beviser ikke', 'Indkomster kan være forskellige']) {
    assert.ok(field.help.includes(phrase), phrase);
  }
  const sources = schema.source_groups[field.source_group].map(id => schema.source_objects[id]);
  for (const [role, reference] of [['source', '2021/1284'], ['dependency_source', '2019/935'],
    ['guidance', 'oid=1977388'], ['guidance', 'medlemskab-af-folkekirken']]) {
    assert.ok(sources.some(s => s.role === role && JSON.stringify(s).includes(reference)), reference);
  }
});
