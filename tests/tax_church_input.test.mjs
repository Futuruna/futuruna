// Real contract projection, no tax implementation, network or private records.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
const enabled = { skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build.' };
function run(args) {
  const p = spawnSync(binary, args, { cwd: root, encoding: 'utf8', timeout: 600000,
    maxBuffer: 64 * 1024 * 1024, env: { ...process.env, FUTURUNA_CALCULATION_JOBS: '1' } });
  assert.ifError(p.error); assert.equal(p.status, 0, p.stderr);
  return p.stdout;
}

for (const [model, entry, prefix] of [
  ['personskat.calculate.runa', 'beregn_personskat', ''],
  ['personskat-par14.calculate.runa', 'beregn_personskat_delår', 'personskat.'],
]) {
  test(`${entry}: income-year church guidance follows taxpayer and spouse`, enabled, () => {
    const schema = JSON.parse(run(['schema', `examples/danish-income-tax/${model}`,
      '--entry', entry, '--format', 'compact-json']));
    const fields = ['', 'ægtefælle.MedÆgtefælle.fakta.'].map(person => {
      const path = `${prefix}${person}lønmodtager.betaler_kirkeskat`;
      const found = schema.field_metadata.filter(f => f.path === path);
      assert.equal(found.length, 1, path);
      return found[0];
    });
    assert.equal(fields[0].help, fields[1].help);
    for (const field of fields) {
      assert.equal(field.anchor, 'PersonskatLønmodtagerInput');
      assert.match(field.question, /hele indkomståret, eller slet ikke/);
      for (const phrase of ['medlemskab i dag', 'true betyder', 'false betyder', 'nul',
        'ukendt status', 'ind-/udmeldelse', 'må der ikke gættes', 'PSL § 14', 'ikke indkomst']) {
        assert.ok(field.help.includes(phrase), `${field.path}: ${phrase}`);
      }
      const sources = schema.source_groups[field.source_group].map(id => schema.source_objects[id]);
      for (const url of ['https://www.borger.dk/kultur-og-fritid/medlemskab-af-folkekirken',
        'https://www.dst.dk/da/Statistik/dokumentation/Times/personindkomst/kiskat']) {
        assert.ok(sources.some(s => s.role === 'guidance' && JSON.stringify(s).includes(url)), url);
      }
      assert.ok(sources.some(s => s.role === 'warning'
        && s.data.value.includes('opdager ikke disse forhold automatisk')));
    }
  });
}

test('calculation assessment actually emits the church coverage warning without claiming a new guard', enabled, () => {
  const output = run(['tests/personskat_church_warning_test.runa']);
  assert.match(output, /kirkeskat_forbehold_bevares/);
  assert.match(output, /kirkeskat_forbehold_er_ikke_en_ny_kontrol/);
});
