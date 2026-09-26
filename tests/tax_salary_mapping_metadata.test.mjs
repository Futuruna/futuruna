// Inspect real typed metadata, not an assumed interpretation of comment text.
// No compiler build, network or personal documents.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const binary = process.env.FUTURUNA_MODEL_TEST_RUNA;

test('salary mapping links both helpers to payroll guidance and unresolved source obligations', {
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
  const label = 'personskat_aarsopgoerelse_løn';
  const anchors = meta.anchors.filter(anchor => anchor.label === label);
  assert.equal(anchors.length, 1);
  assert.equal(anchors[0].references.length, 1);
  const reference = anchors[0].references[0];
  const attachments = reference.attachments;
  assert.deepEqual(attachments.map(item => item.role), ['guidance', 'guidance', 'warning']);
  function field(item, name) {
    const fields = item.data.arguments.filter(arg => arg.field === name);
    assert.equal(fields.length, 1);
    assert.equal(fields[0].value.kind, 'string');
    return fields[0].value.value;
  }
  const [am, payroll, warning] = attachments;
  assert.equal(field(am, 'url'), 'https://skat.dk/borger/am-bidrag');
  assert.equal(field(payroll, 'url'), 'https://info.skat.dk/data.aspx?oid=2233519');
  assert.match(field(payroll, 'identifier'), /felt 13/);
  assert.equal(warning.type, 'PersonskatÅrsopgørelseMappingAdvarsel');
  const message = field(warning, 'besked');
  for (const phrase of ['felt 13', 'korrigeret årsbeløb', 'forskelsbeløb',
    'samlet personlig indkomst', 'egen ATP', 'bortseelsesberettiget',
    'ikke privat pension', 'andre indkomstgrene', 'kildebaseret',
    'DirekteMapping beviser ikke', 'fortegn og øre uden afrunding',
    'ikke et krav om et nyt dokument', 'fra-bilag-til-input.md#lønlinjer-kræver-klassifikation']) {
    assert.ok(message.includes(phrase), phrase);
  }
  const spans = meta.spans.filter(span => span.label === label);
  assert.equal(spans.length, 1);
  assert.deepEqual(spans[0].symbols.map(symbol => [symbol.kind, symbol.name]), [
    ['rule', 'personskat_aarsopgoerelse_kortlaeg_bruttoløn'],
    ['rule', 'personskat_aarsopgoerelse_kortlaeg_ordinær_årsløn'],
  ]);
  for (const item of attachments) {
    assert.ok(reference.typed_values.some(value => value.type === item.type
      && value.path === item.value_path), `${item.type}: typed descendant`);
  }

  const correction = meta.anchors.find(anchor => anchor.label === 'personskat_aarsopgoerelse_lønrettelser');
  assert.ok(correction);
  assert.equal(correction.references.length, 1);
  const correctionSources = correction.references[0].attachments;
  assert.deepEqual(correctionSources.map(item => item.role), ['guidance', 'guidance', 'guidance', 'warning']);
  assert.deepEqual(correctionSources.slice(0, 3).map(item => field(item, 'url')), [
    'https://info.skat.dk/data.aspx?oid=1976766',
    'https://info.skat.dk/data.aspx?oid=2386660',
    'https://info.skat.dk/data.aspx?oid=2386664',
  ]);
  const correctionWarning = field(correctionSources[3], 'besked');
  for (const phrase of ['negative forskelsbeløb', 'kun kontrol', 'oprindeligt indkomstår',
    'retserhvervet tilbagebetalingskrav', 'betalingsdatoen afgør ikke året',
    'Senere opstået', 'ændrer ingen skattekreditter', 'opretter intet ekstra fradrag',
    'fortegn og øre bevares uden afrunding']) assert.ok(correctionWarning.includes(phrase), phrase);
  assert.ok(meta.spans.find(span => span.label === correction.label)?.symbols.some(symbol =>
    symbol.kind === 'rule' && symbol.name === 'personskat_aarsopgoerelse_kortlaeg_rettet_årsløn'));
});
