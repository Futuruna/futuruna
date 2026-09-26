// Actual source metadata; no personal documents, network or compiler build.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;

test('refund mapping exposes typed law, warning and both source-linked helpers', {
  skip: !binary && 'Set FUTURUNA_MODEL_TEST_RUNA; no compiler build.',
}, () => {
  const result = spawnSync(binary, ['meta', '--json', '--type',
    'PersonskatÅrsopgørelseMappingAdvarsel',
    'examples/danish-income-tax/personskat-aarsopgoerelse-kildemapping.runa'], {
    cwd: root, encoding: 'utf8', timeout: 120000, maxBuffer: 2 * 1024 * 1024,
  });
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stderr);
  const meta = JSON.parse(result.stdout);
  assert.equal(meta.schema, 'futuruna.meta.v1');
  assert.deepEqual(meta.diagnostics, []);
  const label = 'personskat_aarsopgoerelse_tilbagebetaling';
  const anchors = meta.anchors.filter(anchor => anchor.label === label);
  assert.equal(anchors.length, 1);
  assert.equal(anchors[0].references.length, 1);
  const reference = anchors[0].references[0];
  const attachments = reference.attachments;
  assert.deepEqual(attachments.map(item => item.role), ['source', 'warning']);
  function field(item, name) {
    const fields = item.data.arguments.filter(arg => arg.field === name);
    assert.equal(fields.length, 1);
    assert.equal(fields[0].value.kind, 'string');
    return fields[0].value.value;
  }
  const [source, warning] = attachments;
  assert.equal(source.type, 'PersonskatÅrsopgørelseKildeInfo');
  assert.equal(field(source, 'url'), 'https://www.retsinformation.dk/eli/lta/2024/460/pdf');
  assert.match(field(source, 'identifier'), /§§ 55, 60, stk\. 3, 62 og 62 A/);
  assert.equal(warning.type, 'PersonskatÅrsopgørelseMappingAdvarsel');
  const message = field(warning, 'besked');
  for (const phrase of ['ikke automatisk', 'tilbagebetalt_par55_øre', 'indkomstår',
    'dobbeltfradrag', 'DirekteMapping beviser ikke', 'Bevar original linje og fortegn']) {
    assert.ok(message.includes(phrase), phrase);
  }
  const spans = meta.spans.filter(span => span.label === label);
  assert.equal(spans.length, 1);
  assert.deepEqual(spans[0].symbols.map(symbol => [symbol.kind, symbol.name]), [
    ['rule', 'personskat_aarsopgoerelse_kortlaeg_tidligere_udbetalt_overskydende_skat'],
    ['rule', 'personskat_aarsopgoerelse_kortlaeg_tilbagebetalt_forskudsskat_par55'],
  ]);
  for (const item of attachments) {
    assert.ok(reference.typed_values.some(value => value.type === item.type
      && value.path === item.value_path), `${item.type}: typed descendant`);
  }
});
