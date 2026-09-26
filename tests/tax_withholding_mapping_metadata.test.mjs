// Actual metadata indexing; no private reports, network or compiler build.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;
test('withholding mapping exposes law, classification boundaries and both helper links', {
  skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build.',
}, () => {
  const result = spawnSync(binary, ['meta', '--json', '--type', 'PersonskatÅrsopgørelseMappingAdvarsel',
    'examples/danish-income-tax/personskat-aarsopgoerelse-kildemapping.runa'], {
    cwd: root, encoding: 'utf8', timeout: 120000, maxBuffer: 2 * 1024 * 1024,
  });
  assert.ifError(result.error); assert.equal(result.status, 0, result.stderr);
  const meta = JSON.parse(result.stdout);
  assert.equal(meta.schema, 'futuruna.meta.v1');
  assert.deepEqual(meta.diagnostics, []);
  const label = 'personskat_aarsopgoerelse_indeholdelse';
  const anchors = meta.anchors.filter(anchor => anchor.label === label);
  assert.equal(anchors.length, 1);
  assert.equal(anchors[0].references.length, 1);
  const reference = anchors[0].references[0];
  assert.deepEqual(reference.attachments.map(item => item.role), ['source', 'warning']);
  const text = JSON.stringify(reference.attachments);
  for (const phrase of ['2024/460', '§ 60, stk. 1, litra a', 'samme person',
    'arbejdsgiver- og månedsrækker', 'afstemmes, ikke summeres oveni',
    'Manglende AM er ikke nul', 'ikke gæt ud fra etiketten', 'eksakte øre',
    'negative rettelsesposter', 'DirekteMapping beviser ikke',
    'ikke krav om et ekstra bilag', 'personskat-skattekreditter.md']) {
    assert.ok(text.includes(phrase), phrase);
  }
  const spans = meta.spans.filter(span => span.label === label);
  assert.equal(spans.length, 1);
  assert.deepEqual(spans[0].symbols.filter(s => s.kind === 'rule').map(s => s.name), [
    'personskat_aarsopgoerelse_kortlaeg_a_skat_og_indeholdt_am',
    'personskat_aarsopgoerelse_kortlaeg_afklaret_indeholdelse',
  ]);
  for (const item of reference.attachments) {
    assert.ok(reference.typed_values.some(value => value.type === item.type && value.path === item.value_path));
  }
});
