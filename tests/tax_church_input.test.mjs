// Real contract projection, no tax implementation, network or private records.
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
const enabled = { skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build.' };
function run(args, expectedStatus = 0) {
  // A cold full contract exceeded ten wall-clock minutes on the shared 8 GB
  // host. Keep one calculation worker and allow completion, not weaker checks.
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 1200000,
    maxBuffer: 64 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, expectedStatus, p.stderr);
  return p.stdout;
}

for (const [model, entry, prefixes] of [
  ['personskat.calculate.runa', 'beregn_personskat', ['']],
  ['personskat-par14.calculate.runa', 'beregn_personskat_delår',
    ['personskat.', 'helårsgrundlag.DokumenteretHelårsPersonskat.personskat.']],
]) {
  test(`${entry}: income-year church guidance follows taxpayer and spouse`, enabled, () => {
    const schema = JSON.parse(run(['schema', `examples/danish-income-tax/${model}`,
      '--entry', entry, '--format', 'compact-json']));
    const fields = prefixes.flatMap(prefix => ['', 'ægtefælle.MedÆgtefælle.fakta.'].map(person => {
      const path = `${prefix}${person}lønmodtager.kirkeskat`;
      const found = schema.field_metadata.filter(f => f.path === path);
      assert.equal(found.length, 1, path);
      return found[0];
    }));
    for (const field of fields) {
      assert.equal(field.help, fields[0].help);
      assert.equal(field.anchor, 'PersonskatLønmodtagerInput');
      assert.match(field.question, /hele indkomståret, en del af året, slet ikke, eller er det uafklaret/);
      for (const phrase of ['medlemskab i dag', 'IngenKirkeskatHeleÅret', 'KirkeskatHeleÅret', 'nul',
        'KirkeskatUoplyst', 'KirkeskatEnDelAfÅret', 'tilbageholder', 'PSL § 14', 'ikke indkomst']) {
        assert.ok(field.help.includes(phrase), `${field.path}: ${phrase}`);
      }
      const sources = schema.source_groups[field.source_group].map(id => schema.source_objects[id]);
      for (const url of ['https://www.borger.dk/kultur-og-fritid/medlemskab-af-folkekirken',
        'https://www.dst.dk/da/Statistik/dokumentation/Times/personindkomst/kiskat']) {
        assert.ok(sources.some(s => s.role === 'guidance' && JSON.stringify(s).includes(url)), url);
      }
      assert.ok(sources.some(s => s.role === 'warning'
        && s.data.value.includes('KirkeskatUoplyst og KirkeskatEnDelAfÅret tilbageholder')));
    }
  });
}

test('the four source statuses retain their coverage boundary and diagnostic bridge', enabled, () => {
  const output = run(['tests/personskat_church_warning_test.runa']);
  assert.match(output, /kirkeskat_forbehold_bevares/);
  assert.match(output, /kirkeskat_forbehold_bevarer_kontrolstatus/);
  assert.match(output, /kun_afklarede_helårsstatusser_understøttes/);
  assert.match(output, /helårsbro_bevarer_medlem_og_diagnostik/);
});

test('canonical main and spouse comparisons withhold unknown or part-year church status', enabled, () => {
  const model = 'examples/danish-income-tax/personskat.calculate.runa';
  const envelope = JSON.parse(run(['template', model, '--format', 'json']));
  assert.deepEqual(envelope.cases[0].input.lønmodtager.kirkeskat, { $variant: 'KirkeskatUoplyst' },
    'generated placeholders must not assert known nonmembership');
  assert.ok(!Object.hasOwn(envelope.cases[0].input.lønmodtager, 'betaler_kirkeskat'));
  const base = buildFictionalCases(envelope).envelope.cases[0].input;
  const cases = [];
  for (const spouse of [false, true]) {
    for (const status of ['IngenKirkeskatHeleÅret', 'KirkeskatHeleÅret', 'KirkeskatUoplyst', 'KirkeskatEnDelAfÅret']) {
      const input = structuredClone(base);
      let person = input;
      if (spouse) {
        const fakta = Object.fromEntries(['lønmodtager', 'kapitalindkomst', 'aktieavance', 'udenlandske_sociale_bidrag',
          'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter'].map(key => [key, structuredClone(input[key])]));
        input.ægtefælle = { $variant: 'MedÆgtefælle', fakta, samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] };
        person = fakta;
      }
      person.lønmodtager.kirkeskat = { $variant: status };
      cases.push({ case_id: `${spouse ? 'spouse' : 'main'}-${status}`, input, spouse,
        valid: status === 'IngenKirkeskatHeleÅret' || status === 'KirkeskatHeleÅret' });
    }
  }
  envelope.cases = cases.map(({ case_id, input }) => ({ case_id, input }));
  const evidence = mkdtempSync(join(tmpdir(), 'futuruna-church-status-'));
  console.log(`Fictional church-status evidence: ${evidence}`);
  const file = join(evidence, 'cases.json');
  writeFileSync(file, JSON.stringify(envelope), { flag: 'wx', mode: 0o600 });
  const output = JSON.parse(run(['call', model, '--input', file]));
  writeFileSync(join(evidence, 'results.json'), JSON.stringify(output), { flag: 'wx', mode: 0o600 });
  assert.deepEqual(output.diagnostics, []);
  assert.equal(output.results.length, cases.length);
  for (const [i, row] of output.results.entries()) {
    const expected = cases[i], a = row.result.vurdering;
    assert.equal(row.case_id, expected.case_id);
    assert.equal(a.alle_kontroller_gyldige, expected.valid, row.case_id);
    assert.equal(a.slutskat_til_sammenligning_øre, expected.valid ? row.result.slutskat_øre : null, row.case_id);
    const path = `${expected.spouse ? 'ægtefælle.MedÆgtefælle.fakta.' : ''}lønmodtager.kirkeskat`;
    const check = a.kontroller.find(c => c.sti === path);
    assert.equal(check?.gyldig, expected.valid, path);
    if (!expected.valid) assert.ok(a.fejl.some(f => f.sti === path && f.forklaring.includes('modelgrænse')));
  }
  // Existing independently recorded ordinary private-pension observation.
  assert.equal(output.results[0].result.vurdering.slutskat_til_sammenligning_øre, 19661194);
  assert.ok(output.results[1].result.vurdering.slutskat_til_sammenligning_øre > 19661194);

  envelope.cases = ['legacy-boolean', 'boolean-in-new-field', 'unknown-constructor'].map(case_id => {
    const input = structuredClone(base);
    if (case_id === 'legacy-boolean') {
      delete input.lønmodtager.kirkeskat; input.lønmodtager.betaler_kirkeskat = false;
    } else input.lønmodtager.kirkeskat = case_id === 'boolean-in-new-field' ? false : { $variant: 'OpdigtetKirkestatus' };
    return { case_id, input };
  });
  const badFile = join(evidence, 'rejected-shapes.json');
  writeFileSync(badFile, JSON.stringify(envelope), { flag: 'wx', mode: 0o600 });
  const rejected = JSON.parse(run(['call', model, '--input', badFile], 1));
  writeFileSync(join(evidence, 'rejected-results.json'), JSON.stringify(rejected), { flag: 'wx', mode: 0o600 });
  assert.deepEqual(rejected.results, []);
  assert.deepEqual(new Set(rejected.diagnostics.map(d => d.case_id)), new Set(envelope.cases.map(c => c.case_id)));
});

test('an unsupported documented annual basis cannot escape the final part-year gate', enabled, () => {
  const model = 'examples/danish-income-tax/personskat-par14.calculate.runa', entry = 'beregn_personskat_delår';
  const v = $variant => ({ $variant });
  const envelope = JSON.parse(run(['template', model, '--entry', entry, '--format', 'json']));
  const person = buildFictionalCases({ cases: [{ input: envelope.cases[0].input.personskat }] }).envelope.cases[0].input;
  person.lønmodtager.bruttoløn_kroner = 300000;
  person.lønmodtager.pension.pbl18_indbetalinger = [];
  // The source below uses day-proportional annualization: 300000 * 365 / 181
  // rounded to whole kroner, not an assumed six-month doubling.
  const annual = structuredClone(person); annual.lønmodtager.bruttoløn_kroner = 604972;
  const base = { personskat: person, skattepligtsændring: v('FuldSkattepligtOphører'),
    skattepligtsperiode: { fra_dato: { år: 2025, måned: 1, dag: 1 }, til_dato: { år: 2025, måned: 6, dag: 30 } },
    valg_afgivet_ved_oplysninger: false, omvalg_dato: null,
    kilder: [{ identifikation: 'fiktiv-loen', beregningsfelt: v('Par14Bruttoløn'), delårsbeløb_kroner: 300000,
      omregningsmetode: v('Par14ForholdsmæssigtLøbendeBeløb'), faktisk_helårsbeløb_kroner: null }],
    helårsgrundlag: { $variant: 'DokumenteretHelårsPersonskat', personskat: annual } };
  const statuses = ['IngenKirkeskatHeleÅret', 'KirkeskatUoplyst', 'KirkeskatEnDelAfÅret'];
  const addSpouse = p => {
    const fakta = Object.fromEntries(['lønmodtager', 'kapitalindkomst', 'aktieavance', 'udenlandske_sociale_bidrag',
      'cfc', 'skatteforhold', 'underskudsforhold', 'ejendomsskatter'].map(key => [key, structuredClone(p[key])]));
    p.ægtefælle = { $variant: 'MedÆgtefælle', fakta, samlevende_ved_indkomstårets_udløb: true, kildeskat25a_fordelinger: [] };
    return fakta;
  };
  const cases = [];
  for (const spouse of [false, true]) {
    for (const status of statuses) {
      const input = structuredClone(base);
      if (spouse) addSpouse(input.personskat);
      const target = spouse ? addSpouse(input.helårsgrundlag.personskat) : input.helårsgrundlag.personskat;
      target.lønmodtager.kirkeskat = v(status);
      cases.push({ case_id: `annual-${spouse ? 'spouse' : 'main'}-${status}`, input,
        periodValid: true, annualValid: status === 'IngenKirkeskatHeleÅret' });
    }
  }
  for (const status of statuses.slice(1)) {
    const input = structuredClone(base);
    addSpouse(input.personskat).lønmodtager.kirkeskat = v(status);
    addSpouse(input.helårsgrundlag.personskat);
    cases.push({ case_id: `period-spouse-${status}`, input, periodValid: false, annualValid: true });
  }
  envelope.cases = cases.map(({ case_id, input }) => ({ case_id, input }));
  const evidence = mkdtempSync(join(tmpdir(), 'futuruna-church-annual-basis-'));
  console.log(`Fictional documented annual-basis evidence: ${evidence}`);
  const file = join(evidence, 'cases.json');
  writeFileSync(file, JSON.stringify(envelope), { flag: 'wx', mode: 0o600 });
  const output = JSON.parse(run(['call', model, '--entry', entry, '--input', file]));
  writeFileSync(join(evidence, 'results.json'), JSON.stringify(output), { flag: 'wx', mode: 0o600 });
  assert.deepEqual(output.diagnostics, []);
  assert.deepEqual(output.results.map(row => row.case_id), cases.map(c => c.case_id));
  for (const [i, { case_id, result: r }] of output.results.entries()) {
    const { periodValid, annualValid } = cases[i], valid = periodValid && annualValid;
    assert.equal(r.delårsresultat.vurdering.alle_kontroller_gyldige, periodValid, case_id);
    assert.equal(r.helårsgrundlag_gyldigt, annualValid, case_id);
    assert.equal(r.vurdering.alle_kontroller_gyldige, valid, case_id);
    assert.equal(r.vurdering.slutskat_til_sammenligning_øre, valid ? 10383101 : null, case_id);
    if (!valid) assert.ok(r.vurdering.fejl.some(f => f.sti === (annualValid ? 'personskat' : 'helårsgrundlag')), case_id);
    if (!periodValid) assert.ok(r.delårsresultat.vurdering.fejl.some(f => f.sti === 'ægtefælle.MedÆgtefælle.fakta.lønmodtager.kirkeskat'), case_id);
  }
});
